use crate::mmu::MogiMMU;
use mogi_system::components::restore::Restorable;
use mogi_system::errors::generic::GenericError;

impl Restorable for MogiMMU {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}
