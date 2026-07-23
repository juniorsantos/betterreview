use betterreview::process::{CommandError, CommandRunner, CommandSpec, TokioCommandRunner};
use rustix::process::{Pid, Signal, kill_process, test_kill_process};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn read_pid(path: &Path) -> Pid {
    let raw = fs::read_to_string(path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    Pid::from_raw(raw).unwrap()
}

async fn wait_until_gone(pid: Pid, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while test_kill_process(pid).is_ok() {
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    true
}

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
    let state = tempfile::tempdir().unwrap();
    let runner = TokioCommandRunner;
    let error = runner
        .run(CommandSpec {
            program: PathBuf::from("tests/fixtures/bin/wait-forever"),
            args: vec![state.path().as_os_str().to_os_string()],
            stdin: None,
            cwd: None,
            timeout: Duration::from_millis(500),
            env: BTreeMap::new(),
            env_remove: Vec::new(),
        })
        .await
        .unwrap_err();
    assert!(matches!(error, CommandError::Timeout { .. }));

    let parent = read_pid(&state.path().join("parent.pid"));
    let descendant = read_pid(&state.path().join("descendant.pid"));
    let parent_gone = wait_until_gone(parent, Duration::from_secs(2)).await;
    let descendant_gone = wait_until_gone(descendant, Duration::from_secs(2)).await;

    if !parent_gone {
        let _ = kill_process(parent, Signal::KILL);
    }
    if !descendant_gone {
        let _ = kill_process(descendant, Signal::KILL);
    }

    assert!(parent_gone);
    assert!(descendant_gone);
}

#[tokio::test]
async fn timeout_covers_blocked_stdin() {
    let runner = TokioCommandRunner;
    let configured_timeout = Duration::from_millis(100);
    let run = runner.run(CommandSpec {
        program: PathBuf::from("tests/fixtures/bin/do-not-read"),
        args: Vec::new(),
        stdin: Some(vec![b'x'; 1024 * 1024]),
        cwd: None,
        timeout: configured_timeout,
        env: BTreeMap::new(),
        env_remove: Vec::new(),
    });

    let error = tokio::time::timeout(Duration::from_secs(2), run)
        .await
        .expect("runner exceeded external watchdog")
        .unwrap_err();

    assert!(matches!(
        error,
        CommandError::Timeout { timeout } if timeout == configured_timeout
    ));
}
