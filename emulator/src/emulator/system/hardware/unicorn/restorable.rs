use std::collections::BTreeMap;

use unicorn_engine::{
    Context,
    ContextMode,
    Unicorn,
};

use crate::emulator::abstractions::errors::{
    GenericError,
    GenericErrorKind,
};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::hardware::unicorn::{
    Snapshot,
    UCWrapper,
};

impl Restorable for UCWrapper<'_> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        let context = self.uc_mut().context_alloc()?;
        let saved_contexts = clone_contexts(&self.contexts, self.uc.as_mut().unwrap())?;
        self.snapshots.push(Snapshot {
            uc_context: context,
            contexts: saved_contexts,
            next_key: self.next_key,
        });
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
        Ok(())
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        let snapshot = self
            .snapshots
            .last()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot to restore"))?;
        self.uc
            .as_mut()
            .unwrap()
            .ctl_context_mode(ContextMode::CPU | ContextMode::Memory)?;
        let result = restore_helper(self.uc.as_mut().unwrap(), snapshot);
        self.uc
            .as_mut()
            .unwrap()
            .ctl_context_mode(ContextMode::CPU)?;
        result?;
        self.next_key = snapshot.next_key;
        self.contexts = clone_contexts(&snapshot.contexts, self.uc.as_mut().unwrap())?;
        Ok(())
    }
}
fn clone_contexts(
    contexts: &BTreeMap<u64, Context>,
    uc: &mut Unicorn<'_, *mut UCWrapper<'_>>,
) -> Result<BTreeMap<u64, Context>, GenericError> {
    // Create a temporary context to which we save the current state.
    let mut tmp_context = uc.context_alloc()?;
    uc.context_save(&mut tmp_context)?;
    // Save all contexts to a new context that we can later copy over again.
    let cloned_contexts = contexts
        .iter()
        .zip(
            (0..contexts.len())
                .map(|_| uc.context_alloc())
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map(|((index, context), mut new_context)| {
            uc.context_restore(context);
            uc.context_save(&mut new_context);
            (*index, new_context)
        })
        .collect::<BTreeMap<_, _>>();
    // Restore the original state
    uc.context_restore(&tmp_context)?;
    Ok(cloned_contexts)
}

fn restore_helper(
    uc: &mut Unicorn<'_, *mut UCWrapper<'_>>,
    snapshot: &Snapshot,
) -> Result<(), GenericError> {
    uc.context_restore(&snapshot.uc_context)?;
    uc.ctl_flush_tlb()?;
    Ok(())
}
