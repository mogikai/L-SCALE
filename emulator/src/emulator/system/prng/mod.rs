use std::fmt;
use std::collections::BTreeMap;

use rand::{Rng, RngCore, SeedableRng};

use crate::emulator::{abstractions::{restore::Restorable, system::SystemControl}, platform::linux::kernel::task_id::TaskId};

#[derive(Debug, Default)]
pub struct SeedablePrngMap<Prng, TaskId>
where
    Prng: Rng + SeedableRng,
    TaskId: Ord + Copy + Clone + Default
{
    seed: Seed<Prng>,
    prngs: BTreeMap<TaskId, Prng>
}

impl<Prng, TaskId> SeedablePrngMap<Prng, TaskId>
where
    Prng: Rng + SeedableRng,
    TaskId: Ord + Copy + Clone + Default
{
    pub fn new(seed: Prng::Seed) -> Self {
        Self {
            seed: Seed::<Prng> { seed },
            prngs: BTreeMap::new()
        }
    }
}

impl<Prng, TaskId> Restorable for SeedablePrngMap<Prng, TaskId>
where
    Prng: Rng + SeedableRng,
    TaskId: Ord + Copy + Clone + Default
{
    fn take_snapshot(&mut self) -> Result<(), crate::emulator::abstractions::errors::GenericError> {
        todo!()
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), crate::emulator::abstractions::errors::GenericError> {
        todo!()
    }

    fn restore_snapshot(&mut self) -> Result<(), crate::emulator::abstractions::errors::GenericError> {
        todo!()
    }
}

#[derive(Default)]
pub struct Seed<R>
where
    R: SeedableRng
{
    seed: <R as SeedableRng>::Seed
}

impl<R> fmt::Debug for Seed<R>
where
    R: SeedableRng
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
