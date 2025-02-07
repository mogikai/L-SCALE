use crate::emulator::abstractions::errors::{
    GenericError,
    GenericErrorKind,
};
use crate::emulator::abstractions::file_descriptors::{
    FileDescriptorError,
    FileDescriptorOps,
    FileOperationsReady,
};
use crate::emulator::abstractions::network::{
    ShutdownMethod,
    SocketAddress,
};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::fs::Whence;
use crate::emulator::platform::linux::kernel::net::sockopt::Sockopt;
use crate::emulator::platform::linux::kernel::socket::Optval;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::filesystem::Stat;
use std::cell::{
    Ref,
    RefCell,
};
use std::cmp::min;
use std::rc::Rc;
use tracing::{
    info,
    trace,
};

#[derive(Debug, Default, Clone)]
pub struct StdioSnapshot {
    stdin_data: Vec<u8>,
    pos: usize,
}

#[derive(Debug)]
pub struct Stdio {
    snapshots: Vec<StdioSnapshot>,
    data: StdioSnapshot,
}

impl Restorable for Stdio {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.push(self.data.clone());
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        if self.snapshots.len() <= 1 {
            Err(GenericError::new(GenericErrorKind::Unknown, "No previous snapshot available"))?;
        }
        self.snapshots.pop();
        self.restore_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        self.data
            .clone_from(self.snapshots.last().unwrap_or(&Default::default()));
        Ok(())
    }
}

impl Stdio {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            data: Default::default(),
        }
    }

    pub fn pos(&self) -> usize {
        self.data.pos
    }

    pub fn stdin_read(
        &mut self,
        count: u64,
    ) -> Option<Vec<u8>> {
        if self.data.stdin_data.is_empty() || self.data.pos >= self.data.stdin_data.len() {
            return None;
        }
        let read_end = min(count as usize, self.data.stdin_data.len());
        let data = self.data.stdin_data[self.data.pos..read_end].to_owned();
        self.data.pos = read_end;
        Some(data)
    }

    pub fn add_stdin_data(
        &mut self,
        data: &[u8],
    ) {
        self.data.stdin_data.extend(data);
    }

    pub fn lseek(
        &mut self,
        whence: Whence,
        offset: i64,
    ) -> usize {
        match whence {
            Whence::SEEK_SET => self.data.pos = offset as usize,
            Whence::SEEK_CUR => {
                self.data.pos = if offset.is_negative() {
                    self.data.pos.wrapping_sub(offset.abs() as usize)
                } else {
                    self.data.pos.wrapping_add(offset.abs() as usize)
                }
            }
            Whence::SEEK_END => {
                unimplemented!()
            }
        }
        self.data.pos
    }
}

#[derive(Debug, Clone)]
pub struct StdioData {
    stdio: Rc<RefCell<Stdio>>,
}

impl StdioData {
    pub fn new(stdio: Rc<RefCell<Stdio>>) -> Self {
        Self { stdio }
    }
}

#[derive(Debug)]
pub struct StdInputOps {
    stat: RefCell<Stat>,
}

impl StdInputOps {
    pub fn new() -> Self {
        Self {
            stat: Default::default(),
        }
    }
}

impl FileDescriptorOps for StdInputOps {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
        interest: FileOperationsReady,
    ) -> Option<FileOperationsReady> {
        todo!()
    }

    fn supports_poll(
        &self,
        fd: &mut Self::D,
    ) -> bool {
        todo!()
    }

    fn close(
        &self,
        fd: &mut Self::D,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn stat<'a>(
        &'a self,
        fd: &'a mut Self::D,
    ) -> Result<Ref<'a, Stat>, FileDescriptorError> {
        Ok(self.stat.borrow())
    }

    fn read(
        &self,
        fd: &mut Self::D,
        count: u64,
    ) -> Result<Option<Vec<u8>>, FileDescriptorError> {
        let stdio = fd.stdio.as_mut().ok_or(FileDescriptorError::NotSupported)?;
        Ok(stdio.stdio.borrow_mut().stdin_read(count))
    }

    fn sendto(
        &self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<SocketAddress>,
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), FileDescriptorError> {
        todo!()
    }

    fn connect(
        &self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn getsockopt(
        &self,
        fd: &mut Self::D,
        sockopt: Sockopt,
    ) -> Result<Optval, FileDescriptorError> {
        todo!()
    }

    fn accept(
        &self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, SocketAddress), FileDescriptorError> {
        todo!()
    }

    fn shutdown(
        &self,
        fd: &mut Self::D,
        how: ShutdownMethod,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }
}

#[derive(Debug)]
pub struct StdOutputOps {
    stat: RefCell<Stat>,
}

impl StdOutputOps {
    pub fn new() -> Self {
        Self {
            stat: Default::default(),
        }
    }
}

impl FileDescriptorOps for StdOutputOps {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
        interest: FileOperationsReady,
    ) -> Option<FileOperationsReady> {
        todo!()
    }

    fn supports_poll(
        &self,
        fd: &mut Self::D,
    ) -> bool {
        todo!()
    }

    fn close(
        &self,
        fd: &mut Self::D,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn stat<'a>(
        &'a self,
        fd: &'a mut Self::D,
    ) -> Result<Ref<'a, Stat>, FileDescriptorError> {
        Ok(self.stat.borrow())
    }

    fn write(
        &self,
        fd: &mut Self::D,
        data: &[u8],
    ) -> Result<usize, FileDescriptorError> {
        trace!("STDOUT/ERR: {:}", String::from_utf8_lossy(data));
        Ok(data.len())
    }

    fn sendto(
        &self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<SocketAddress>,
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), FileDescriptorError> {
        todo!()
    }

    fn connect(
        &self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn getsockopt(
        &self,
        fd: &mut Self::D,
        sockopt: Sockopt,
    ) -> Result<Optval, FileDescriptorError> {
        todo!()
    }

    fn accept(
        &self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, SocketAddress), FileDescriptorError> {
        todo!()
    }

    fn shutdown(
        &self,
        fd: &mut Self::D,
        how: ShutdownMethod,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }
}
