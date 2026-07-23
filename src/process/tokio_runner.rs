use async_trait::async_trait;
use rustix::process::{Pid, Signal, kill_process_group};
use std::process::Stdio;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    process::{Child, Command},
};

use super::{CommandError, CommandOutput, CommandRunner, CommandSpec};

#[derive(Debug, Clone, Copy, Default)]
pub struct TokioCommandRunner;

async fn terminate_process_group(child: &mut Child, pid: Pid) {
    let _ = kill_process_group(pid, Signal::KILL);
    let _ = child.start_kill();
    let _ = child.wait().await;
}

fn missing_pipe(name: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        format!("child {name} was not piped"),
    )
}

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
        command.process_group(0);

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
        let pid = child
            .id()
            .and_then(|raw| Pid::from_raw(raw as i32))
            .ok_or_else(|| CommandError::Io(std::io::Error::other("child has no process id")))?;
        let child_stdin = child.stdin.take();
        let Some(mut child_stdout) = child.stdout.take() else {
            terminate_process_group(&mut child, pid).await;
            return Err(CommandError::Io(missing_pipe("stdout")));
        };
        let Some(mut child_stderr) = child.stderr.take() else {
            terminate_process_group(&mut child, pid).await;
            return Err(CommandError::Io(missing_pipe("stderr")));
        };

        let operation = async {
            let write_stdin = async move {
                match (spec.stdin, child_stdin) {
                    (Some(input), Some(mut stdin)) => {
                        stdin.write_all(&input).await?;
                        stdin.shutdown().await
                    }
                    (Some(_), None) => Err(missing_pipe("stdin")),
                    (None, _) => Ok(()),
                }
            };
            let read_stdout = async move {
                let mut output = Vec::new();
                child_stdout.read_to_end(&mut output).await?;
                Ok::<_, std::io::Error>(output)
            };
            let read_stderr = async move {
                let mut output = Vec::new();
                child_stderr.read_to_end(&mut output).await?;
                Ok::<_, std::io::Error>(output)
            };

            let (_, stdout, stderr, status) =
                tokio::try_join!(write_stdin, read_stdout, read_stderr, child.wait())?;
            Ok::<_, std::io::Error>((status, stdout, stderr))
        };

        let (status, stdout, stderr) = match tokio::time::timeout(spec.timeout, operation).await {
            Ok(Ok(output)) => output,
            Ok(Err(source)) => {
                terminate_process_group(&mut child, pid).await;
                return Err(CommandError::Io(source));
            }
            Err(_) => {
                terminate_process_group(&mut child, pid).await;
                return Err(CommandError::Timeout {
                    timeout: spec.timeout,
                });
            }
        };

        Ok(CommandOutput {
            status: status.code().unwrap_or(-1),
            stdout,
            stderr,
        })
    }
}
