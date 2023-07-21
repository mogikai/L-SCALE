use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::memory::Memory;

impl<Endianness: ByteOrder> Restorable for Memory<Endianness> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}
