mod tokio_runner;

use async_trait::async_trait;
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, time::Duration};

pub use tokio_runner::TokioCommandRunner;

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub stdin: Option<Vec<u8>>,
    pub cwd: Option<PathBuf>,
    pub timeout: Duration,
    pub env: BTreeMap<OsString, OsString>,
    pub env_remove: Vec<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("failed to spawn {program}: {source}")]
    Spawn {
        program: PathBuf,
        source: std::io::Error,
    },
    #[error("command timed out after {timeout:?}")]
    Timeout { timeout: Duration },
    #[error("command I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

#[async_trait]
pub trait CommandRunner: Send + Sync {
    async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError>;
}
