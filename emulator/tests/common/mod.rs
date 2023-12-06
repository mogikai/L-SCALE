mod emulate;
mod mocks;

pub use emulate::emulate;
use fuzzyemu::emulator::interface::error::{
    SchedulerError,
    StopReason,
};
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
    fmt,
    EnvFilter,
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
    return r;
}

pub fn prepare_binary(bin: &str) {
    let sysdir = get_sysdir();
    let homedir = sysdir.join(Path::new("home/user"));
    let pwd = env::current_dir().unwrap();
    let assetdir = pwd.join(Path::new("tests/assets"));
    if !homedir.exists() {
        fs::create_dir_all(&homedir).unwrap();
    }
    env::set_current_dir(homedir).unwrap();
    let _ret = Command::new("make")
        .arg("-f")
        .arg(format!("{}/Makefile", assetdir.display()))
        .arg(bin)
        .env("VPATH", format!("{}", assetdir.display()))
        .env("CC", "clang-13")
        .env(
            "CFLAGS",
            format!("--sysroot {} -B {} -fuse-ld={} -pie", sysdir.display(), "/usr/lib/llvm-13/bin", "lld"),
        )
        .env("LDFLAGS", "-Wl,-allow-shlib-undefined -fPIC")
        .status()
        .unwrap();
    env::set_current_dir(&pwd).unwrap();
}

#[derive(Debug)]
pub enum TestFailure {
    ProcessFailure(StopReason),
    SchedulerFailure(SchedulerError),
    Unknown,
}

impl From<SchedulerError> for TestFailure {
    fn from(e: SchedulerError) -> Self {
        Self::SchedulerFailure(e)
    }
}
