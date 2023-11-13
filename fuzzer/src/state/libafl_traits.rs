use std::cell::{
    Ref,
    RefMut,
};
use std::time::Duration;

use crate::state::FUZEState;
use libafl::corpus::{
    Corpus,
    CorpusId,
    HasTestcase,
    Testcase,
};
use libafl::inputs::{
    Input,
    UsesInput,
};
use libafl::monitors::ClientPerfMonitor;
use libafl::state::{
    HasClientPerfMonitor,
    HasCorpus,
    HasExecutions,
    HasImported,
    HasLastReportTime,
    HasMaxSize,
    HasMetadata,
    HasNamedMetadata,
    HasRand,
    HasSolutions,
    State,
    UsesState,
};
use libafl_bolts::bolts_prelude::NamedSerdeAnyMap;
use libafl_bolts::rands::Rand;
use libafl_bolts::serdeany::SerdeAnyMap;
use libafl_bolts::Error;
use serde::de::DeserializeOwned;
use serde::{
    Deserializer,
    Serialize,
    Serializer,
};

impl<InputT, CorpusT, RandomT, SolutionsT> UsesInput for FUZEState<InputT, CorpusT, RandomT, SolutionsT>
where
    InputT: Input,
{
    type Input = InputT;
}

impl<InputT, CorpusT, RandomT, SolutionsT> State for FUZEState<InputT, CorpusT, RandomT, SolutionsT>
where
    CorpusT: Corpus<Input = Self::Input>,
    RandomT: Rand,
    SolutionsT: Corpus<Input = Self::Input>,
    Self: UsesInput,
    InputT: Serialize + DeserializeOwned,
{
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasClientPerfMonitor for FUZEState<InputT, CorpusT, RandomT, SolutionsT> {
    fn introspection_monitor(&self) -> &ClientPerfMonitor {
        unimplemented!()
    }

    fn introspection_monitor_mut(&mut self) -> &mut ClientPerfMonitor {
        unimplemented!()
    }
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasCorpus for FUZEState<InputT, CorpusT, RandomT, SolutionsT>
where
    InputT: Input,
    CorpusT: Corpus<Input = Self::Input>,
{
    type Corpus = CorpusT;

    fn corpus(&self) -> &Self::Corpus {
        &self.corpus
    }

    fn corpus_mut(&mut self) -> &mut Self::Corpus {
        &mut self.corpus
    }
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasTestcase for FUZEState<InputT, CorpusT, RandomT, SolutionsT>
where
    InputT: Input,
    Self: HasCorpus,
{
    fn testcase(
        &self,
        id: CorpusId,
    ) -> Result<Ref<Testcase<<Self as UsesInput>::Input>>, Error> {
        Ok(self.corpus().get(id)?.borrow())
    }

    fn testcase_mut(
        &self,
        id: CorpusId,
    ) -> Result<RefMut<Testcase<<Self as UsesInput>::Input>>, Error> {
        Ok(self.corpus().get(id)?.borrow_mut())
    }
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasExecutions for FUZEState<InputT, CorpusT, RandomT, SolutionsT> {
    fn executions(&self) -> &usize {
        &self.executions
    }

    fn executions_mut(&mut self) -> &mut usize {
        &mut self.executions
    }
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasImported for FUZEState<InputT, CorpusT, RandomT, SolutionsT> {
    fn imported(&self) -> &usize {
        &self.imported
    }

    fn imported_mut(&mut self) -> &mut usize {
        &mut self.imported
    }
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasRand for FUZEState<InputT, CorpusT, RandomT, SolutionsT>
where
    RandomT: Rand,
{
    type Rand = RandomT;

    fn rand(&self) -> &Self::Rand {
        &self.rand
    }

    fn rand_mut(&mut self) -> &mut Self::Rand {
        &mut self.rand
    }
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasSolutions for FUZEState<InputT, CorpusT, RandomT, SolutionsT>
where
    InputT: Input,
    SolutionsT: Corpus<Input = InputT>,
{
    type Solutions = SolutionsT;

    fn solutions(&self) -> &Self::Solutions {
        &self.solutions
    }

    fn solutions_mut(&mut self) -> &mut Self::Solutions {
        &mut self.solutions
    }
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasMaxSize for FUZEState<InputT, CorpusT, RandomT, SolutionsT> {
    fn max_size(&self) -> usize {
        self.max_size
    }

    fn set_max_size(
        &mut self,
        max_size: usize,
    ) {
        self.max_size = max_size
    }
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasMetadata for FUZEState<InputT, CorpusT, RandomT, SolutionsT> {
    fn metadata_map(&self) -> &SerdeAnyMap {
        &self.metadata
    }

    fn metadata_map_mut(&mut self) -> &mut SerdeAnyMap {
        &mut self.metadata
    }
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasLastReportTime for FUZEState<InputT, CorpusT, RandomT, SolutionsT> {
    fn last_report_time(&self) -> &Option<Duration> {
        &self.last_report_time
    }

    fn last_report_time_mut(&mut self) -> &mut Option<Duration> {
        &mut self.last_report_time
    }
}

impl<InputT, CorpusT, RandomT, SolutionsT> HasNamedMetadata for FUZEState<InputT, CorpusT, RandomT, SolutionsT> {
    fn named_metadata_map(&self) -> &NamedSerdeAnyMap {
        &self.named_metadata
    }

    fn named_metadata_map_mut(&mut self) -> &mut NamedSerdeAnyMap {
        &mut self.named_metadata
    }
}
