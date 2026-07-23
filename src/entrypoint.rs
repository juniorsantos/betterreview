use async_trait::async_trait;
use std::{env, sync::Arc};

use crate::{
    cli::{Cli, LaunchRequest, ProviderArg},
    context::{ContextError, ContextResolver, ResolveRequest, ResolvedContext},
    doctor::Doctor,
    domain::ProviderKind,
    process::{CommandRunner, TokioCommandRunner},
};

#[derive(Debug)]
pub enum ResolvedLaunch {
    Review(ResolvedContext),
    Resume(Option<String>),
    Sessions,
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error("failed to resolve the current directory: {0}")]
    CurrentDirectory(#[source] std::io::Error),
    #[error("application runtime is not installed yet")]
    RuntimeNotInstalled,
}

#[async_trait]
pub trait LaunchBackend: Send + Sync {
    async fn launch(&self, launch: ResolvedLaunch) -> Result<(), LaunchError>;
}

struct PendingRuntime;

#[async_trait]
impl LaunchBackend for PendingRuntime {
    async fn launch(&self, _launch: ResolvedLaunch) -> Result<(), LaunchError> {
        Err(LaunchError::RuntimeNotInstalled)
    }
}

pub async fn run(cli: Cli) -> Result<(), LaunchError> {
    let runner: Arc<dyn CommandRunner> = Arc::new(TokioCommandRunner);
    run_with(cli, runner, &PendingRuntime).await
}

pub async fn run_with(
    cli: Cli,
    runner: Arc<dyn CommandRunner>,
    backend: &dyn LaunchBackend,
) -> Result<(), LaunchError> {
    match cli.launch_request() {
        LaunchRequest::Doctor { provider, host } => {
            let report = Doctor::new(runner)
                .check(provider.map(provider_kind), host.as_deref())
                .await;
            print!("{report}");
            Ok(())
        }
        LaunchRequest::Review {
            target,
            provider,
            host,
            repository,
        } => {
            let resolved = ContextResolver::new(runner)
                .resolve(ResolveRequest {
                    cwd: env::current_dir().map_err(LaunchError::CurrentDirectory)?,
                    target,
                    provider_hint: provider.map(provider_kind),
                    host_hint: host,
                    repository_hint: repository,
                })
                .await?;
            backend.launch(ResolvedLaunch::Review(resolved)).await
        }
        LaunchRequest::Resume(session_id) => {
            backend.launch(ResolvedLaunch::Resume(session_id)).await
        }
        LaunchRequest::Sessions => backend.launch(ResolvedLaunch::Sessions).await,
    }
}

fn provider_kind(provider: ProviderArg) -> ProviderKind {
    match provider {
        ProviderArg::GitHub => ProviderKind::GitHub,
        ProviderArg::GitLab => ProviderKind::GitLab,
    }
}
