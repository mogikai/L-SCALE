use std::collections::BTreeMap;

use mogi_system::errors::generic::{
    GenericError,
    GenericErrorKind,
};
use unicorn_engine::{
    Context,
    ContextMode,
    Unicorn,
};

use crate::emulator::system::hardware::unicorn::{
    Snapshot,
    UCWrapper,
};
use mogi_system::components::restore::Restorable;

impl Restorable for UCWrapper<'_> {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        let context = self
            .uc_mut()
            .context_alloc()
            .map_err(|_| GenericError::unknown())?;
        let saved_contexts = clone_contexts(&self.contexts, self.uc.as_mut().unwrap())?;
        self.snapshots.push(Snapshot {
            uc_context: context,
            contexts: saved_contexts,
            next_key: self.next_key,
        });
        self.uc_mut()
            .ctl_set_context_mode(ContextMode::CPU | ContextMode::MEMORY);
        self.uc
            .as_mut()
            .unwrap()
            .context_save(&mut self.snapshots.last_mut().unwrap().uc_context)
            .map_err(|_| GenericError::unknown())?;
        self.uc_mut().ctl_set_context_mode(ContextMode::CPU);
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        if self.snapshots.len() <= 1 {
            Err(GenericError::new(GenericErrorKind::Unknown, "No previous snapshot available"))?;
        }
        self.restore_snapshot()?;
        self.snapshots.pop();
        self.restore_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        let snapshot = self
            .snapshots
            .last()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot to restore"))?;
        self.uc
            .as_mut()
            .unwrap()
            .ctl_set_context_mode(ContextMode::CPU | ContextMode::MEMORY)
            .map_err(|_| GenericError::unknown())?;
        let result = restore_helper(self.uc.as_mut().unwrap(), snapshot);
        self.uc
            .as_mut()
            .unwrap()
            .ctl_set_context_mode(ContextMode::CPU)
            .map_err(|_| GenericError::unknown())?;
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
    let mut tmp_context = uc.context_alloc().map_err(|_| GenericError::unknown())?;
    uc.context_save(&mut tmp_context)
        .map_err(|_| GenericError::unknown())?;
    // Save all contexts to a new context that we can later copy over again.
    let cloned_contexts = contexts
        .iter()
        .zip(
            (0..contexts.len())
                .map(|_| uc.context_alloc())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| GenericError::unknown())?,
        )
        .map(|((index, context), mut new_context)| {
            uc.context_restore(context);
            uc.context_save(&mut new_context);
            (*index, new_context)
        })
        .collect::<BTreeMap<_, _>>();
    // Restore the original state
    uc.context_restore(&tmp_context)
        .map_err(|_| GenericError::unknown())?;
    Ok(cloned_contexts)
}

fn restore_helper(
    uc: &mut Unicorn<'_, *mut UCWrapper<'_>>,
    snapshot: &Snapshot,
) -> Result<(), GenericError> {
    uc.context_restore(&snapshot.uc_context)
        .map_err(|_| GenericError::unknown())?;
    uc.ctl_flush_tlb().map_err(|_| GenericError::unknown())?;
    Ok(())
}
