use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::hardware::unicorn::UCWrapper;
use unicorn_engine::ContextMode;

impl Restorable for UCWrapper<'_> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        let mut context = self.uc_mut().context_alloc()?;
        self.uc_mut().context_save(&mut context)?;
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        self.restore();
        todo!("discard")
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.uc_mut().ctl_flush_tlb();
        todo!()
    }
}
