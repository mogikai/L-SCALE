mod emulate;
mod mocks;

pub use emulate::emulate;
use mogi_system::errors::SchedulerError;
use mogi_system::errors::cpu::StopReason;
use samples;
use std::path::{
    Path,
    PathBuf,
};
use std::process::Command;
use std::sync::Once;
use std::{
    env,
    fs,
};
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

pub fn get_sysdir() -> PathBuf {
    let mut r = PathBuf::from(samples::systemdir());
    r.push(Path::new("ubuntu_23.04"));
    r
}

pub fn prepare_binary(bin: &str) -> Result<(), BuildError> {
    let sysdir = get_sysdir();
    let homedir = sysdir.join(Path::new("home/user"));
    let pwd = env::current_dir()?;
    let assetdir = pwd.join(Path::new("tests/assets"));
    if !homedir.exists() {
        fs::create_dir_all(&homedir)?;
    }
    let ret = Command::new("make")
        .arg("-f")
        .arg(format!("{}/Makefile", assetdir.display()))
        .arg(bin)
        .env("VPATH", format!("{}", assetdir.display()))
        .env("CC", "clang-15")
        .env(
            "CFLAGS",
            format!("--sysroot {} -B {} -fuse-ld={} -pie", sysdir.display(), "/usr/lib/llvm-15/bin", "lld"),
        )
        .env("LDFLAGS", "-Wl,-allow-shlib-undefined -fPIC")
        .current_dir(homedir)
        .status()?;
    if !ret.success() {
        match ret.code() {
            Some(code) => return Err(BuildError::RetCode(code)),
            None => return Err(BuildError::Crash),
        }
    }
    return Ok(());
}

#[derive(Debug)]
#[allow(unused)]
pub enum BuildError {
    IO(std::io::Error),
    RetCode(i32),
    Crash,
}

impl From<std::io::Error> for BuildError {
    fn from(item: std::io::Error) -> Self {
        Self::IO(item)
    }
}

#[derive(Debug)]
#[allow(unused)]
pub enum TestFailure {
    ProcessFailure(StopReason),
    SchedulerFailure(SchedulerError),
    PrepareError(BuildError),
    Unknown,
}

impl From<SchedulerError> for TestFailure {
    fn from(e: SchedulerError) -> Self {
        Self::SchedulerFailure(e)
    }
}

impl From<BuildError> for TestFailure {
    fn from(e: BuildError) -> Self {
        Self::PrepareError(e)
    }
}
