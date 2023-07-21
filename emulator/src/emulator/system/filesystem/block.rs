use std::cmp::Ordering;

use crate::emulator::system::filesystem;
use crate::emulator::system::filesystem::{FSError, SnapId};

#[derive(Eq, Copy, Clone, Debug)]
pub(super) struct Block {
    pub(super) start: usize,
    pub(super) size: usize,
    pub(super) data_offset: usize,
    pub(super) snap_id: SnapId,
}

impl Block {
    pub(super) fn new(
        start: usize,
        size: usize,
        data_offset: usize,
        snap_id: SnapId,
    ) -> Result<Self, FSError> {
        if !filesystem::is_4k_aligned(start)
            || !filesystem::is_4k_aligned(size)
            || !filesystem::is_4k_aligned(data_offset)
        {
            return Err(FSError::Unknown);
        }
        Ok(Self {
            start,
            size,
            data_offset,
            snap_id,
        })
    }

    pub(super) fn end(&self) -> usize {
        self.start + self.size
    }

    pub(super) fn contains(
        &self,
        pos: usize,
    ) -> bool {
        self.start <= pos && pos <= self.start + self.size
    }

    pub(super) fn has_space(
        &self,
        pos: usize,
        size: usize,
    ) -> bool {
        self.contains(pos) && self.contains(pos + size)
    }

    pub(super) fn split(
        self,
        start: usize,
        end: usize,
    ) -> Result<(Option<Block>, Block, Option<Block>), FSError> {
        if !filesystem::is_4k_aligned(start) || !filesystem::is_4k_aligned(end) {
            return Err(FSError::Unknown);
        }
        assert!(self.start <= start);
        assert!(end <= self.end());
        let front_size = start - self.start;
        let back_size = self.end() - end;
        let front = if front_size > 0 {
            Some(Block::new(self.start, front_size, self.data_offset, self.snap_id)?)
        } else {
            None
        };

        let center = Block::new(
            self.start + front_size,
            self.size - front_size - back_size,
            self.data_offset + front_size,
            self.snap_id,
        )?;

        let back = if back_size > 0 {
            Some(Block::new(end, back_size, self.data_offset + front_size + center.size, self.snap_id)?)
        } else {
            None
        };
        Ok((front, center, back))
    }
}

impl PartialEq for Block {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.start.eq(&other.start) || self.data_offset.eq(&other.data_offset)
    }
}

impl PartialOrd for Block {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Block {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl std::borrow::Borrow<usize> for Block {
    fn borrow(&self) -> &usize {
        &self.start
    }
}
