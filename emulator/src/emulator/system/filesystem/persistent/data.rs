use std::cmp::min;
use std::collections::{
    BTreeMap,
    BTreeSet,
};
use std::mem::MaybeUninit;
use std::ptr;

use addr_align::Align4k;

use crate::emulator::system::filesystem::FSError;
use crate::emulator::system::filesystem::persistent::block::Block;

#[derive(Debug)]
pub(super) struct Data {
    data: Vec<u8>,
    free_blocks: Vec<BTreeMap<usize, Vec<Block>>>,
    current_snap: usize,
}

impl Data {
    pub(super) fn new() -> Self {
        Self {
            data: Vec::new(),
            free_blocks: vec![BTreeMap::new()],
            current_snap: 0,
        }
    }

    pub(super) fn write_to_block_cow(
        &mut self,
        pos: usize,
        data: &[u8],
        contents: &mut BTreeSet<Block>,
        block_start: usize,
    ) -> Result<usize, FSError> {
        let block = contents
            .take(&block_start)
            .expect("The block_starts were constructed beforehand and must exist");
        // We can now split the block if COW is necessary, or modify directly and reinsert
        let block = if block.snap_id < self.current_snap {
            assert!(block.contains(pos));
            let end = if block.contains(pos + data.len()) {
                pos + data.len()
            } else {
                block.end()
            };
            let (front, mut center, back) = block.split(pos.align4k_down(), end.align4k_up())?;
            front.map(|b| contents.insert(b));
            back.map(|b| contents.insert(b));
            let new_data_offset = self.data.len();
            self.data
                .extend_from_within(block.data_offset..block.data_offset + block.size);
            center.data_offset = new_data_offset;
            center
        } else {
            block
        };

        assert!(block.contains(pos));
        let block_offset = block.data_offset + (pos - block.start);
        let amount = min(block.end() - pos, data.len());
        self.data[block_offset..block_offset + amount].copy_from_slice(&data[..amount]);

        contents.insert(block);
        Ok(amount)
    }

    /// Inserts a new block at the end of the existing blocks and writes data into the block.
    /// If pos is after the end of the last block, the area in between will be allocated for
    /// this block and filled with zeros. This guarantees that blocks are always continuous.
    pub(super) fn new_block(
        &mut self,
        pos: usize,
        contents: &mut BTreeSet<Block>,
        data: &[u8],
    ) -> Result<(), FSError> {
        // TODO: First search for unused existing blocks
        let last_block_end = contents.iter().last().map_or(0, |b| b.end());
        assert!(pos >= last_block_end);
        let fill = pos - last_block_end;
        let alignment = (data.len() + fill).align4k_up() - data.len() - fill;
        let block = Block::new(last_block_end, data.len() + fill + alignment, self.data.len(), self.current_snap)?;
        self.data.resize(self.data.len() + fill, 0);
        self.data.extend_from_slice(data);
        self.data.resize(self.data.len() + alignment, 0);
        contents.insert(block);
        Ok(())
    }

    /// Attempts to read as much data as possible from the supplied block at position pos
    ///
    pub(super) fn read_block(
        &self,
        pos: usize,
        block: &Block,
        buffer: &mut [MaybeUninit<u8>],
    ) -> Result<usize, FSError> {
        if !block.contains(pos) {
            return Err(FSError::NoAddress);
        }
        let to_read = min(buffer.len(), block.end() - pos);
        let offset = block.data_offset + (pos - block.start);
        unsafe {
            ptr::copy_nonoverlapping(
                self.data[offset..offset + to_read].as_ptr(),
                buffer[..to_read].as_mut_ptr().cast(),
                to_read,
            )
        };
        // buffer[..to_read].copy_from_slice(&self.data[offset..offset + to_read]);
        Ok(to_read)
    }

    pub(super) fn save_free_blocks(
        &mut self,
        blocks: impl ExactSizeIterator<Item = Block>,
    ) {
        self.free_blocks.reserve(blocks.len());
        blocks.for_each(|b| {
            // We only want to save blocks that are used in the current snapshot.
            // Old blocks are copied anyways on write and need to be preserved.
            if b.snap_id == self.current_snap {
                self.free_blocks[self.current_snap]
                    .entry(b.size)
                    .or_default()
                    .push(b);
            }
        });
    }
    pub(super) fn restore_and_discard(
        &mut self,
        snap_marker: DataSnapMarker,
    ) {
        self.restore(snap_marker);
        self.current_snap -= 1;
    }

    pub(super) fn restore(
        &mut self,
        snap_marker: DataSnapMarker,
    ) {
        self.free_blocks.truncate(self.current_snap);
        self.free_blocks.push(BTreeMap::new());
        self.data.truncate(snap_marker.data_marker);
    }

    pub(super) fn snapshot(&mut self) -> DataSnapMarker {
        self.free_blocks.push(BTreeMap::new());
        self.current_snap += 1;
        DataSnapMarker {
            data_marker: self.data.len(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DataSnapMarker {
    data_marker: usize,
}
