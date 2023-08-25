use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::hardware::unicorn::{Snapshot, UCWrapper};
use unicorn_engine::Context;
use unicorn_engine::ContextMode;

impl Restorable for UCWrapper<'_> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        let context = self.uc_mut().context_alloc()?;
        self.snapshots.push(Snapshot { uc_context: context });
        self.uc_mut()
            .ctl_context_mode(ContextMode::CPU | ContextMode::Memory);
        self.uc
            .as_mut()
            .unwrap()
            .context_save(&mut self.snapshots.last_mut().unwrap().uc_context)?;
        self.uc_mut().ctl_context_mode(ContextMode::CPU);
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        self.restore()?;
        self.snapshots.pop();
        return Ok(());
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.uc_mut()
            .ctl_context_mode(ContextMode::CPU | ContextMode::Memory);
        let context: &Context = &self.snapshots.last()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot to restor")?;
        self.uc().context_restore(context);
        self.uc_mut().ctl_context_mode(ContextMode::CPU);
        self.uc_mut().ctl_flush_tlb();
        return Ok(());
    }
}
