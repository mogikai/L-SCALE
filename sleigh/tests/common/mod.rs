use mogi_sleigh::compiler::SleighCompilerResult;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::preprocessor::SleighPreprocessorFactory;
use mogi_sleigh::spec::ProcessorSpecification;
use std::path::Path;
use std::sync::Once;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    EnvFilter,
    fmt,
};

static INIT: Once = Once::new();

pub fn setup_test() {
    INIT.call_once(|| {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_env("FUZE_LOG"))
            .init();
    })
}

pub fn build(path: String) -> SleighCompilerResult<ProcessorSpecification> {
    let spec_path = Path::new(&path);
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    compiler.compile(spec_path)
}
