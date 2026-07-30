use crate::{
    context::{ContextError, RemoteUrl, parse_remote_url},
    process::{CommandOutput, CommandRunner, CommandSpec},
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};

pub(crate) struct GitContext {
    pub root: PathBuf,
    pub remote_name: String,
    pub remote: RemoteUrl,
}

pub(crate) async fn discover(
    runner: &dyn CommandRunner,
    cwd: &Path,
    remote_hint: Option<&str>,
) -> Result<Option<GitContext>, ContextError> {
    let root = successful_output(runner, git_command(cwd, ["rev-parse", "--show-toplevel"])).await;
    let Some(root) = root.and_then(|output| text(&output)) else {
        return Ok(None);
    };
    let root = PathBuf::from(root);
    let remotes = successful_output(runner, git_command(&root, ["remote"])).await;
    let Some(remotes) = remotes.and_then(|output| text(&output)) else {
        return Ok(None);
    };
    let available = remotes
        .lines()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let available_names = available_names(&available);
    let remote_name = match remote_hint {
        Some(remote) if available.iter().any(|name| name == remote) => remote.to_owned(),
        Some(remote) => {
            return Err(ContextError::RemoteNotFound {
                remote: remote.into(),
                available: available_names,
            });
        }
        None if available.iter().any(|name| name == "origin") => "origin".into(),
        None if available.len() == 1 => available[0].clone(),
        None if available.is_empty() => return Ok(None),
        None => {
            return Err(ContextError::AmbiguousRemote {
                available: available_names,
            });
        }
    };
    let remote = successful_output(
        runner,
        git_command(&root, ["remote", "get-url", remote_name.as_str()]),
    )
    .await
    .and_then(|output| text(&output))
    .and_then(|url| parse_remote_url(&url).ok())
    .ok_or_else(|| ContextError::InvalidRemote {
        remote: remote_name.clone(),
        available: available_names,
    })?;
    Ok(Some(GitContext {
        root,
        remote_name,
        remote,
    }))
}

pub(crate) async fn current_branch(runner: &dyn CommandRunner, root: &Path) -> Option<String> {
    let output = successful_output(runner, git_command(root, ["branch", "--show-current"])).await?;
    let branch = text(&output)?;
    (!branch.is_empty()).then_some(branch)
}

async fn successful_output(runner: &dyn CommandRunner, spec: CommandSpec) -> Option<CommandOutput> {
    let output = runner.run(spec).await.ok()?;
    (output.status == 0).then_some(output)
}

fn git_command<const N: usize>(cwd: &Path, args: [&str; N]) -> CommandSpec {
    let mut command_args = vec![OsString::from("-C"), cwd.as_os_str().to_owned()];
    command_args.extend(args.into_iter().map(OsString::from));
    CommandSpec {
        program: PathBuf::from("git"),
        args: command_args,
        stdin: None,
        cwd: None,
        timeout: Duration::from_secs(5),
        env: BTreeMap::new(),
        env_remove: Vec::new(),
    }
}

fn text(output: &CommandOutput) -> Option<String> {
    let value = String::from_utf8(output.stdout.clone()).ok()?;
    Some(value.trim().into())
}

fn available_names(remotes: &[String]) -> String {
    if remotes.is_empty() {
        "(none)".into()
    } else {
        remotes.join(", ")
    }
}
