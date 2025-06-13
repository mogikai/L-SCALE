use crate::api::error::FuzzerError;
use crate::api::observer::HasObservation;
use crate::api::reducer::Reducer;
use crate::core::luzzer::Observation;
use std::cell::RefCell;
use std::rc::Rc;
use tlsh::{
    GeneratorType,
    TlshGenerator,
};

#[derive(Debug)]
pub struct BlockHitLshReducer<T> {
    observer: Rc<RefCell<T>>,
}

impl<T> BlockHitLshReducer<T> {
    pub fn new(observer: Rc<RefCell<T>>) -> Self {
        Self { observer }
    }
}

impl<T> Reducer for BlockHitLshReducer<T>
where
    T: HasObservation<Observation = [u8]>,
{
    type Output = Observation;

    fn reduce(&mut self) -> Result<Self::Output, FuzzerError> {
        let observer = self.observer.borrow();
        let observation = &*observer.observation();
        let mut generator = TlshGenerator::new();
        generator.update(observation);
        Ok(Observation::new(generator.finalize().unwrap()))
    }
}
