use async_trait::async_trait;
use std::process::Stdio;
use tokio::{io::AsyncWriteExt as _, process::Command};

use super::{CommandError, CommandOutput, CommandRunner, CommandSpec};

#[derive(Debug, Clone, Copy, Default)]
pub struct TokioCommandRunner;

#[async_trait]
impl CommandRunner for TokioCommandRunner {
    async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .envs(&spec.env);

        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        for key in &spec.env_remove {
            command.env_remove(key);
        }

        let mut child = command.spawn().map_err(|source| CommandError::Spawn {
            program: spec.program.clone(),
            source,
        })?;

        if let Some(input) = spec.stdin {
            let mut stdin = child.stdin.take().ok_or_else(|| {
                CommandError::Io(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "child stdin was not piped",
                ))
            })?;
            stdin.write_all(&input).await?;
            stdin.shutdown().await?;
        }

        let output = tokio::time::timeout(spec.timeout, child.wait_with_output())
            .await
            .map_err(|_| CommandError::Timeout {
                timeout: spec.timeout,
            })??;

        Ok(CommandOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}
