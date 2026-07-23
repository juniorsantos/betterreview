use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, sync::Arc, time::Duration};

use crate::process::{CommandOutput, CommandRunner, CommandSpec};

use super::super::ProviderError;

pub(super) struct GlabClient<R> {
    runner: Arc<R>,
}

impl<R> GlabClient<R>
where
    R: CommandRunner,
{
    pub fn new(runner: Arc<R>) -> Self {
        Self { runner }
    }

    pub async fn api<I, S>(
        &self,
        args: I,
        stdin: Option<Vec<u8>>,
        operation: &str,
        timeout: Duration,
    ) -> Result<Vec<u8>, ProviderError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let output = self
            .runner
            .run(CommandSpec {
                program: PathBuf::from("glab"),
                args: args.into_iter().map(Into::into).collect(),
                stdin,
                cwd: None,
                timeout,
                env: BTreeMap::new(),
                env_remove: Vec::new(),
            })
            .await?;
        translate_output(output, operation)
    }
}

fn translate_output(output: CommandOutput, operation: &str) -> Result<Vec<u8>, ProviderError> {
    if output.status == 0 {
        return Ok(output.stdout);
    }
    let message = redact(&output.stderr);
    let lower = message.to_ascii_lowercase();
    if lower.contains("auth") || lower.contains("login") {
        return Err(ProviderError::Authentication { guidance: message });
    }
    if lower.contains("not found") {
        return Err(ProviderError::NotFound {
            resource: operation.into(),
        });
    }
    Err(ProviderError::Permission {
        operation: operation.into(),
        message,
    })
}

fn redact(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if ["token", "authorization", "private-token", "oauth"]
                .iter()
                .any(|pattern| lower.contains(pattern))
            {
                "[REDACTED]".into()
            } else {
                line.into()
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}
