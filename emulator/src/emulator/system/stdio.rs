use crate::emulator::abstractions::file_descriptors::{
    FileDescriptorError,
    FileDescriptorOps,
    FileOperationsReady,
};
use crate::emulator::abstractions::network::{
    ShutdownMethod,
    SocketAddress,
};
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::sockopt::Sockopt;
use crate::emulator::platform::linux::kernel::socket::Optval;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::filesystem::Stat;
use std::cell::{
    Ref,
    RefCell,
};
use std::cmp::min;
use std::collections::VecDeque;
use std::rc::Rc;
use tracing::info;

#[derive(Debug)]
pub struct Stdio {
    stdin_data: VecDeque<u8>,
    yield_reason: bool,
}

impl Stdio {
    pub fn new() -> Self {
        Self {
            stdin_data: VecDeque::new(),
            yield_reason: false,
        }
    }

    pub fn stdin_read(
        &mut self,
        count: u64,
    ) -> Option<Vec<u8>> {
        if self.stdin_data.is_empty() {
            self.yield_reason = true;
            return None;
        }
        let n = min(count as usize, self.stdin_data.len());
        Some(self.stdin_data.drain(0..n).collect())
    }

    pub fn add_stdin_data(
        &mut self,
        data: &[u8],
    ) {
        self.stdin_data.extend(data);
        self.yield_reason = false
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
pub struct StdInputOps {}

impl StdInputOps {
    pub fn new() -> Self {
        Self {}
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
        info!("{:}", String::from_utf8_lossy(data));
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
