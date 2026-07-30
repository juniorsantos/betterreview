use crate::{
    context::{git, parse_change_request_url},
    domain::{ChangeRequestKey, ProviderKind},
    process::{CommandRunner, CommandSpec},
};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, sync::Arc, time::Duration};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ResolveRequest {
    pub cwd: PathBuf,
    pub target: Option<String>,
    pub provider_hint: Option<ProviderKind>,
    pub host_hint: Option<String>,
    pub repository_hint: Option<String>,
    pub remote_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryInput {
    Exact(ChangeRequestKey),
    CurrentBranch {
        provider: ProviderKind,
        host: String,
        repository: String,
        branch: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContext {
    pub discovery: Option<DiscoveryInput>,
    pub show_session_picker: bool,
    pub repository_root: Option<PathBuf>,
    pub remote_name: Option<String>,
}

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("target is neither a review URL nor a numeric review number: {target}")]
    InvalidTarget { target: String },
    #[error(
        "both GitHub and GitLab are available for {host}; pass --provider github or --provider gitlab"
    )]
    AmbiguousProvider { host: String },
    #[error(
        "unable to determine the provider for {host}; pass --provider github or --provider gitlab"
    )]
    ProviderUndetected { host: String },
    #[error(
        "multiple Git remotes are available ({available}); pass --remote with one of these names"
    )]
    AmbiguousRemote { available: String },
    #[error("Git remote '{remote}' was not found; available remotes: {available}")]
    RemoteNotFound { remote: String, available: String },
    #[error("Git remote '{remote}' does not have a valid URL; available remotes: {available}")]
    InvalidRemote { remote: String, available: String },
}

pub struct ContextResolver {
    runner: Arc<dyn CommandRunner>,
}

impl ContextResolver {
    pub fn new(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }

    pub async fn resolve(&self, request: ResolveRequest) -> Result<ResolvedContext, ContextError> {
        if let Some(target) = request.target.as_deref() {
            if let Ok(mut key) = parse_change_request_url(target) {
                key.provider = request.provider_hint.unwrap_or(key.provider);
                key.host = request.host_hint.unwrap_or(key.host);
                key.repository = request.repository_hint.unwrap_or(key.repository);
                return Ok(exact(key));
            }
            if target.parse::<u64>().is_err() {
                return Err(ContextError::InvalidTarget {
                    target: target.into(),
                });
            }
        }

        let number = request
            .target
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(|_| ContextError::InvalidTarget {
                target: request.target.clone().unwrap_or_default(),
            })?;

        if let (Some(number), Some(host), Some(repository)) = (
            number,
            request.host_hint.as_deref(),
            request.repository_hint.as_deref(),
        ) {
            let provider = self
                .resolve_provider(request.provider_hint, None, host)
                .await?;
            return Ok(exact(ChangeRequestKey {
                provider,
                host: host.into(),
                repository: repository.into(),
                number,
            }));
        }

        let Some(git) = git::discover(
            self.runner.as_ref(),
            &request.cwd,
            request.remote_hint.as_deref(),
        )
        .await?
        else {
            return Ok(session_picker(None, None));
        };
        let host = request.host_hint.unwrap_or(git.remote.host);
        let repository = request.repository_hint.unwrap_or(git.remote.repository);
        let provider = self
            .resolve_provider(request.provider_hint, None, &host)
            .await?;

        if let Some(number) = number {
            return Ok(ResolvedContext {
                discovery: Some(DiscoveryInput::Exact(ChangeRequestKey {
                    provider,
                    host,
                    repository,
                    number,
                })),
                show_session_picker: false,
                repository_root: Some(git.root),
                remote_name: Some(git.remote_name),
            });
        }

        let Some(branch) = git::current_branch(self.runner.as_ref(), &git.root).await else {
            return Ok(session_picker(Some(git.root), Some(git.remote_name)));
        };
        Ok(ResolvedContext {
            discovery: Some(DiscoveryInput::CurrentBranch {
                provider,
                host,
                repository,
                branch,
            }),
            show_session_picker: false,
            repository_root: Some(git.root),
            remote_name: Some(git.remote_name),
        })
    }

    async fn resolve_provider(
        &self,
        hint: Option<ProviderKind>,
        path_provider: Option<ProviderKind>,
        host: &str,
    ) -> Result<ProviderKind, ContextError> {
        if let Some(provider) = hint.or(path_provider).or_else(|| public_provider(host)) {
            return Ok(provider);
        }
        let github = self.probe("gh", host).await;
        let gitlab = self.probe("glab", host).await;
        match (github, gitlab) {
            (true, true) => Err(ContextError::AmbiguousProvider { host: host.into() }),
            (true, false) => Ok(ProviderKind::GitHub),
            (false, true) => Ok(ProviderKind::GitLab),
            (false, false) => Err(ContextError::ProviderUndetected { host: host.into() }),
        }
    }

    async fn probe(&self, program: &str, host: &str) -> bool {
        self.runner
            .run(CommandSpec {
                program: PathBuf::from(program),
                args: vec![
                    OsString::from("auth"),
                    OsString::from("status"),
                    OsString::from("--hostname"),
                    OsString::from(host),
                ],
                stdin: None,
                cwd: None,
                timeout: Duration::from_secs(5),
                env: BTreeMap::new(),
                env_remove: Vec::new(),
            })
            .await
            .is_ok_and(|output| output.status == 0)
    }
}

fn exact(key: ChangeRequestKey) -> ResolvedContext {
    ResolvedContext {
        discovery: Some(DiscoveryInput::Exact(key)),
        show_session_picker: false,
        repository_root: None,
        remote_name: None,
    }
}

fn session_picker(
    repository_root: Option<PathBuf>,
    remote_name: Option<String>,
) -> ResolvedContext {
    ResolvedContext {
        discovery: None,
        show_session_picker: true,
        repository_root,
        remote_name,
    }
}

fn public_provider(host: &str) -> Option<ProviderKind> {
    match host {
        "github.com" => Some(ProviderKind::GitHub),
        "gitlab.com" => Some(ProviderKind::GitLab),
        _ => None,
    }
}
