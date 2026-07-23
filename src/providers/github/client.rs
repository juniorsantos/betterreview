use serde_json::Value;
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, sync::Arc, time::Duration};

use crate::process::{CommandOutput, CommandRunner, CommandSpec};

use super::super::ProviderError;

pub(super) struct GhClient<R> {
    runner: Arc<R>,
}

impl<R> GhClient<R>
where
    R: CommandRunner,
{
    pub fn new(runner: Arc<R>) -> Self {
        Self { runner }
    }

    pub async fn graphql(
        &self,
        host: &str,
        query: &str,
        variables: Value,
        operation: &str,
    ) -> Result<Vec<u8>, ProviderError> {
        let stdin = serde_json::to_vec(&serde_json::json!({
            "query": query,
            "variables": variables,
        }))
        .map_err(|error| ProviderError::MalformedResponse {
            operation: operation.into(),
            message: error.to_string(),
        })?;
        self.execute(
            host,
            ["api", "graphql", "--hostname", host, "--input", "-"],
            Some(stdin),
            operation,
        )
        .await
    }

    pub async fn api<I, S>(
        &self,
        host: &str,
        args: I,
        operation: &str,
    ) -> Result<Vec<u8>, ProviderError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.execute(host, args, None, operation).await
    }

    async fn execute<I, S>(
        &self,
        _host: &str,
        args: I,
        stdin: Option<Vec<u8>>,
        operation: &str,
    ) -> Result<Vec<u8>, ProviderError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let output = self
            .runner
            .run(CommandSpec {
                program: PathBuf::from("gh"),
                args: args.into_iter().map(Into::into).collect(),
                stdin,
                cwd: None,
                timeout: Duration::from_secs(60),
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
    if lower.contains("rate limit") {
        return Err(ProviderError::Permission {
            operation: operation.into(),
            message,
        });
    }
    if lower.contains("not found") || output.status == 4 {
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
