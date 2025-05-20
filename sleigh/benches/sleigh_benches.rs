#![feature(get_mut_unchecked)]

use std::num::NonZeroUsize;
use std::path::Path;
use std::rc::Rc;

use criterion::{
    Criterion,
    black_box,
    criterion_group,
    criterion_main,
};
use mogi_sleigh::SleighPreprocessorFactory;
use mogi_sleigh::compiler::SleighCompilerResult;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::lifter::context_provider::{
    ContextProvider,
    SimpleContextProvider,
};
use mogi_sleigh::lifter::source::TargetInstructionSource;
use mogi_sleigh::lifter::{
    BlockEndingOpDecider,
    Lifter,
    LifterConfig,
};
use mogi_sleigh::spec::jir::basic_block::builder::TargetBasicBlockBuilder;
use mogi_sleigh::spec::jir::basic_block::optimizer::{
    OptimizerContext,
    OptimizerOptions,
};
use mogi_sleigh::spec::jir::context_stack::ContextStack;
use mogi_sleigh::spec::jir::jumplist::MatcherResult;
use mogi_sleigh::spec::{
    CustomOpId,
    ProcessorSpecification,
    SpaceId,
};
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::MapKey;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    EnvFilter,
    fmt,
};

struct NonEndingDecider {}
impl BlockEndingOpDecider for NonEndingDecider {
    fn is_block_ending_custom_op(
        &self,
        custom_op_id: CustomOpId,
    ) -> bool {
        false
    }
}

struct MockInstructionSource {
    bytes: Vec<u8>,
}

impl MockInstructionSource {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl TargetInstructionSource for MockInstructionSource {
    type Error = ();

    fn load(
        &self,
        addr: EVAddr,
        buffer: &mut [u8],
    ) -> Result<(), Self::Error> {
        buffer.copy_from_slice(&self.bytes[addr.into()..usize::from(addr) + buffer.len()]);
        Ok(())
    }
}

const ASSETS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/test/");

fn build(path: String) -> SleighCompilerResult<ProcessorSpecification> {
    let spec_path = Path::new(&path);
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    compiler.compile(spec_path)
}

pub fn matching_benchmark(c: &mut Criterion) {
    let path = ASSETS_PATH.to_string() + "x86/x86-64.slaspec";
    let spec = build(path).unwrap();

    let mut context_provider = SimpleContextProvider::default();
    context_provider.set(&spec, "longMode", 1);
    context_provider.set(&spec, "addrsize", 2);
    context_provider.set(&spec, "opsize", 1);

    // pdep rax, rcx, [rbp+8*rsi+0x1234]
    let bytes = b"\xc4\xe2\xf3\xf5\x84\xf5\xcc\xed\xff\xff";

    let match_context = spec.new_match_context(0x0u64.into()).unwrap();

    c.bench_function("match pdep", |b| {
        b.iter(|| {
            let mut context_stack = ContextStack::new(&spec, &context_provider).unwrap();
            match_context
                .r#match(black_box(bytes), &mut context_stack)
                .unwrap();
        })
    });

    let mut context_stack = ContextStack::new(&spec, &context_provider).unwrap();
    let MatcherResult::Some {
        table,
        resolver,
        bytes_consumed,
    } = match_context
        .r#match(black_box(bytes), &mut context_stack)
        .unwrap()
    else {
        panic!("This can't fail :S");
    };

    c.bench_function("build pdep", |b| {
        b.iter(|| {
            let resolver = resolver.clone();

            let mut block_builder = TargetBasicBlockBuilder::new(0x0u64.into());
            block_builder.start_block().unwrap();

            resolver
                .build(&spec, &mut block_builder, &NonEndingDecider {})
                .unwrap();

            block_builder.finish_block().unwrap();
            block_builder.finish(None).unwrap()
        })
    });

    // endbr64
    let bytes = b"\xF3\x0F\x1E\xFA";

    let mut context_stack = ContextStack::new(&spec, &context_provider).unwrap();
    let match_context = spec.new_match_context(0x0u64.into()).unwrap();

    c.bench_function("match endbr64", |b| {
        b.iter(|| {
            let mut context_stack = ContextStack::new(&spec, &context_provider).unwrap();
            match_context
                .r#match(black_box(bytes), &mut context_stack)
                .unwrap();
        })
    });

    let mut context_stack = ContextStack::new(&spec, &context_provider).unwrap();
    let MatcherResult::Some {
        table,
        resolver,
        bytes_consumed,
    } = match_context
        .r#match(black_box(bytes), &mut context_stack)
        .unwrap()
    else {
        panic!("This can't fail :S");
    };

    c.bench_function("build endbr64", |b| {
        b.iter(|| {
            let resolver = resolver.clone();

            let mut block_builder = TargetBasicBlockBuilder::new(0x0u64.into());
            block_builder.start_block().unwrap();

            resolver
                .build(&spec, &mut block_builder, &NonEndingDecider {})
                .unwrap();

            block_builder.finish_block().unwrap();
            block_builder.finish(None).unwrap()
        })
    });

    // vfmaddsub213pd xmm4, xmm1, xmmword ptr [0x1337]
    let bytes = b"\xC4\xE2\xF1\xA6\x24\x25\x37\x13\x00\x00";

    let mut context_stack = ContextStack::new(&spec, &context_provider).unwrap();
    let match_context = spec.new_match_context(0x0u64.into()).unwrap();

    c.bench_function("match vfmaddsub213pd", |b| {
        b.iter(|| {
            let mut context_stack = ContextStack::new(&spec, &context_provider).unwrap();
            match_context
                .r#match(black_box(bytes), &mut context_stack)
                .unwrap();
        })
    });

    let mut context_stack = ContextStack::new(&spec, &context_provider).unwrap();
    let MatcherResult::Some {
        table,
        resolver,
        bytes_consumed,
    } = match_context
        .r#match(black_box(bytes), &mut context_stack)
        .unwrap()
    else {
        panic!("This can't fail :S");
    };

    c.bench_function("build vfmaddsub213pd", |b| {
        b.iter(|| {
            let resolver = resolver.clone();

            let mut block_builder = TargetBasicBlockBuilder::new(0x0u64.into());
            block_builder.start_block().unwrap();

            resolver
                .build(&spec, &mut block_builder, &NonEndingDecider {})
                .unwrap();

            block_builder.finish_block().unwrap();
            block_builder.finish(None).unwrap()
        })
    });
}

pub fn lifting_benchmark(c: &mut Criterion) {
    let path = ASSETS_PATH.to_string() + "x86/x86-64.slaspec";
    let spec = build(path).unwrap();

    let mut context_provider = SimpleContextProvider::default();
    context_provider.set(&spec, "longMode", 1);
    context_provider.set(&spec, "addrsize", 2);
    context_provider.set(&spec, "opsize", 1);

    /*
    endbr64
    push rbp
    lea rcx, [0x00000000]
    lea rax, [0x000392d8]
    movq xmm1, rcx
    movq xmm2, rax
    punpcklqdq xmm1, xmm2
    mov rbp, rsp
    push r15
    push r14
    push r13
    push r12
    push rbx
    sub rsp, 0x88
    mov qword [var_88h], rdi    ; arg1
    movaps xmmword [var_80h], xmm1
    rdtsc
    and byte [0x00038e06], 0xdf ; [0x38e06:1]=0
    lea rsi, [0x00038af0]
    mov r8d, 0x6fffffff
    mov r9d, 0x70000029         ; ')'
    shl rdx, 0x20               ; arg3
    mov qword [0x00038ab0], rcx ; [0x38ab0:8]=0
    mov r14, rcx
    or rax, rdx                 ; arg3
    lea rdx, segment.DYNAMIC    ; 0x37e58
    mov qword [0x00037a60], rax ; [0x37a60:8]=0
    mov rax, qword [segment.DYNAMIC] ; [0x37e58:8]=14
    mov qword [0x00038ac0], rdx ; [0x38ac0:8]=0
    test rax, rax
    jne 0x20277 */

    let block = [
        0xF3, 0x0F, 0x1E, 0xFA, 0x55, 0x48, 0x8D, 0x0D, 0x24, 0xFE, 0xFD, 0xFF, 0x48, 0x8D, 0x05, 0xF5, 0x90, 0x01,
        0x00, 0x66, 0x48, 0x0F, 0x6E, 0xC9, 0x66, 0x48, 0x0F, 0x6E, 0xD0, 0x66, 0x0F, 0x6C, 0xCA, 0x48, 0x89, 0xE5,
        0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x48, 0x81, 0xEC, 0x88, 0x00, 0x00, 0x00, 0x48, 0x89,
        0xBD, 0x78, 0xFF, 0xFF, 0xFF, 0x0F, 0x29, 0x4D, 0x80, 0x0F, 0x31, 0x80, 0x25, 0xEE, 0x8B, 0x01, 0x00, 0xDF,
        0x48, 0x8D, 0x35, 0xD1, 0x88, 0x01, 0x00, 0x41, 0xB8, 0xFF, 0xFF, 0xFF, 0x6F, 0x41, 0xB9, 0x29, 0x00, 0x00,
        0x70, 0x48, 0xC1, 0xE2, 0x20, 0x48, 0x89, 0x0D, 0x7A, 0x88, 0x01, 0x00, 0x49, 0x89, 0xCE, 0x48, 0x09, 0xD0,
        0x48, 0x8D, 0x15, 0x15, 0x7C, 0x01, 0x00, 0x48, 0x89, 0x05, 0x16, 0x78, 0x01, 0x00, 0x48, 0x8B, 0x05, 0x07,
        0x7C, 0x01, 0x00, 0x48, 0x89, 0x15, 0x68, 0x88, 0x01, 0x00, 0x48, 0x85, 0xC0, 0x75, 0x1A, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ];

    let source = MockInstructionSource::new(block.to_vec());
    let config = LifterConfig::new(&NonEndingDecider {}, None, None);

    c.bench_function("lift block", |b| {
        b.iter(|| {
            spec.lift_block(&source, &mut context_provider, &config, 0x0u64.into())
                .unwrap();
        })
    });
}

pub fn optimization_benchmark(c: &mut Criterion) {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .with_env_var("MOGI_LOG")
                .from_env_lossy(),
        )
        .init();

    let path = ASSETS_PATH.to_string() + "x86/x86-64.slaspec";
    let spec = build(path).unwrap();

    let mut context_provider = SimpleContextProvider::default();
    context_provider.set(&spec, "longMode", 1);
    context_provider.set(&spec, "addrsize", 2);
    context_provider.set(&spec, "opsize", 1);

    /*
    endbr64
    push rbp
    lea rcx, [0x00000000]
    lea rax, [0x000392d8]
    movq xmm1, rcx
    movq xmm2, rax
    punpcklqdq xmm1, xmm2
    mov rbp, rsp
    push r15
    push r14
    push r13
    push r12
    push rbx
    sub rsp, 0x88
    mov qword [var_88h], rdi    ; arg1
    movaps xmmword [var_80h], xmm1
    rdtsc
    and byte [0x00038e06], 0xdf ; [0x38e06:1]=0
    lea rsi, [0x00038af0]
    mov r8d, 0x6fffffff
    mov r9d, 0x70000029         ; ')'
    shl rdx, 0x20               ; arg3
    mov qword [0x00038ab0], rcx ; [0x38ab0:8]=0
    mov r14, rcx
    or rax, rdx                 ; arg3
    lea rdx, segment.DYNAMIC    ; 0x37e58
    mov qword [0x00037a60], rax ; [0x37a60:8]=0
    mov rax, qword [segment.DYNAMIC] ; [0x37e58:8]=14
    mov qword [0x00038ac0], rdx ; [0x38ac0:8]=0
    test rax, rax
    jne 0x20277 */

    let block = [
        0xF3, 0x0F, 0x1E, 0xFA, 0x55, 0x48, 0x8D, 0x0D, 0x24, 0xFE, 0xFD, 0xFF, 0x48, 0x8D, 0x05, 0xF5, 0x90, 0x01,
        0x00, 0x66, 0x48, 0x0F, 0x6E, 0xC9, 0x66, 0x48, 0x0F, 0x6E, 0xD0, 0x66, 0x0F, 0x6C, 0xCA, 0x48, 0x89, 0xE5,
        0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x48, 0x81, 0xEC, 0x88, 0x00, 0x00, 0x00, 0x48, 0x89,
        0xBD, 0x78, 0xFF, 0xFF, 0xFF, 0x0F, 0x29, 0x4D, 0x80, 0x0F, 0x31, 0x80, 0x25, 0xEE, 0x8B, 0x01, 0x00, 0xDF,
        0x48, 0x8D, 0x35, 0xD1, 0x88, 0x01, 0x00, 0x41, 0xB8, 0xFF, 0xFF, 0xFF, 0x6F, 0x41, 0xB9, 0x29, 0x00, 0x00,
        0x70, 0x48, 0xC1, 0xE2, 0x20, 0x48, 0x89, 0x0D, 0x7A, 0x88, 0x01, 0x00, 0x49, 0x89, 0xCE, 0x48, 0x09, 0xD0,
        0x48, 0x8D, 0x15, 0x15, 0x7C, 0x01, 0x00, 0x48, 0x89, 0x05, 0x16, 0x78, 0x01, 0x00, 0x48, 0x8B, 0x05, 0x07,
        0x7C, 0x01, 0x00, 0x48, 0x89, 0x15, 0x68, 0x88, 0x01, 0x00, 0x48, 0x85, 0xC0, 0x75, 0x1A, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ];

    let source = MockInstructionSource::new(block.to_vec());
    // let config = LifterConfig::new(&NonEndingDecider {}, Some(NonZeroUsize::new(1).unwrap()), None);
    let config = LifterConfig::new(&NonEndingDecider {}, None, None);

    let optimizer_options = OptimizerOptions::default()
        .with_enable_dse(true)
        .with_enable_copy_coalescing(true)
        .with_enable_constant_folding(true)
        .with_enable_nop_elimination(true);

    let lifted_block = spec
        .lift_block(&source, &mut context_provider, &config, 0x0u64.into())
        .unwrap();

    c.bench_function("optimize block", |b| {
        b.iter_batched(
            || {
                let mut block = lifted_block.0.clone();
                black_box(Rc::make_mut(&mut block));
                block
            },
            |mut block| {
                Rc::make_mut(&mut block)
                    .run_optimization_passes(
                        &OptimizerContext::new(
                            SpaceId::try_from_index(0).unwrap(),
                            SpaceId::try_from_index(1).unwrap(),
                        ),
                        &optimizer_options,
                    )
                    .unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, optimization_benchmark);
criterion_main!(benches);
