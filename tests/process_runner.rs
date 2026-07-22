use betterreview::process::{CommandError, CommandRunner, CommandSpec, TokioCommandRunner};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, time::Duration};

#[tokio::test]
async fn passes_metacharacters_without_shell_expansion() {
    let runner = TokioCommandRunner;
    let output = runner
        .run(CommandSpec {
            program: PathBuf::from("tests/fixtures/bin/echo-args"),
            args: vec![
                OsString::from("$(touch /tmp/must-not-exist)"),
                OsString::from("a; b"),
            ],
            stdin: Some(b"line 1\nline 2".to_vec()),
            cwd: None,
            timeout: Duration::from_secs(2),
            env: BTreeMap::new(),
            env_remove: Vec::new(),
        })
        .await
        .unwrap();
    assert_eq!(output.status, 0);
    let output = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        output,
        "arg=$(touch /tmp/must-not-exist)\narg=a; b\n--stdin--\nline 1\nline 2\n"
    );
}

#[tokio::test]
async fn kills_process_when_timeout_expires() {
    let runner = TokioCommandRunner;
    let error = runner
        .run(CommandSpec {
            program: PathBuf::from("tests/fixtures/bin/wait-forever"),
            args: Vec::new(),
            stdin: None,
            cwd: None,
            timeout: Duration::from_millis(100),
            env: BTreeMap::new(),
            env_remove: Vec::new(),
        })
        .await
        .unwrap_err();
    assert!(matches!(error, CommandError::Timeout { .. }));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        std::process::Command::new("pgrep")
            .args(["-f", "tests/fixtures/bin/wait-forever"])
            .status()
            .map(|status| !status.success())
            .unwrap_or(true)
    );
}
