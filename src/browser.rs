use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, time::Duration};

use url::Url;

use crate::process::{CommandRunner, CommandSpec};

pub(crate) async fn open(runner: &dyn CommandRunner, target: &str) -> Result<(), String> {
    let target = validated_target(target)?;
    let output = runner
        .run(command_spec(&target)?)
        .await
        .map_err(|error| format!("failed to open link: {error}"))?;
    if output.status == 0 {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if detail.is_empty() {
        Err(format!("failed to open link: exit {}", output.status))
    } else {
        Err(format!("failed to open link: {detail}"))
    }
}

fn validated_target(target: &str) -> Result<String, String> {
    let url = Url::parse(target).map_err(|_| "refusing to open an invalid link".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
        return Err("refusing to open an unsafe link".into());
    }
    Ok(url.into())
}

fn command_spec(target: &str) -> Result<CommandSpec, String> {
    #[cfg(target_os = "macos")]
    let (program, args) = ("open", vec![OsString::from(target)]);

    #[cfg(target_os = "linux")]
    let (program, args) = ("xdg-open", vec![OsString::from(target)]);

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err("opening links is unsupported on this platform".into());

    Ok(CommandSpec {
        program: PathBuf::from(program),
        args,
        stdin: None,
        cwd: None,
        timeout: Duration::from_secs(10),
        env: BTreeMap::new(),
        env_remove: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;

    use crate::process::{CommandError, CommandOutput};

    use super::*;

    struct RecordingRunner {
        calls: Mutex<Vec<CommandSpec>>,
        output: CommandOutput,
    }

    #[async_trait]
    impl CommandRunner for RecordingRunner {
        async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError> {
            self.calls.lock().unwrap().push(spec);
            Ok(self.output.clone())
        }
    }

    fn runner(status: i32, stderr: &str) -> RecordingRunner {
        RecordingRunner {
            calls: Mutex::new(Vec::new()),
            output: CommandOutput {
                status,
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
            },
        }
    }

    #[tokio::test]
    async fn opens_http_links_with_the_platform_opener() {
        let runner = runner(0, "");
        let target = "https://github.com/owner/repo/pull/1";

        open(&runner, target).await.unwrap();

        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        #[cfg(target_os = "macos")]
        assert_eq!(calls[0].program, PathBuf::from("open"));
        #[cfg(target_os = "linux")]
        assert_eq!(calls[0].program, PathBuf::from("xdg-open"));
        assert_eq!(calls[0].args, vec![OsString::from(target)]);
    }

    #[tokio::test]
    async fn rejects_non_http_links_without_running_a_command() {
        let runner = runner(0, "");

        let error = open(&runner, "file:///tmp/secret").await.unwrap_err();

        assert_eq!(error, "refusing to open an unsafe link");
        assert!(runner.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reports_opener_failures() {
        let runner = runner(1, "no browser");

        let error = open(&runner, "https://example.com").await.unwrap_err();

        assert_eq!(error, "failed to open link: no browser");
    }
}
