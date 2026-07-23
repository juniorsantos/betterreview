use crate::{
    context::{RemoteUrl, parse_remote_url},
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

pub(crate) async fn discover(runner: &dyn CommandRunner, cwd: &Path) -> Option<GitContext> {
    let root =
        successful_output(runner, git_command(cwd, ["rev-parse", "--show-toplevel"])).await?;
    let root = PathBuf::from(text(&root)?);
    let remotes = successful_output(runner, git_command(&root, ["remote"])).await?;
    let remotes = text(&remotes)?;
    let remote_name = remotes
        .lines()
        .find(|name| *name == "origin")
        .or_else(|| remotes.lines().next())?
        .to_owned();
    let remote = successful_output(
        runner,
        git_command(&root, ["remote", "get-url", remote_name.as_str()]),
    )
    .await?;
    let remote = parse_remote_url(&text(&remote)?).ok()?;
    Some(GitContext {
        root,
        remote_name,
        remote,
    })
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
