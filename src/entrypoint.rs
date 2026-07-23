use async_trait::async_trait;
use std::{collections::BTreeMap, env, sync::Arc};

use crate::{
    cli::{Cli, LaunchRequest, ProviderArg},
    context::{ContextError, ContextResolver, ResolveRequest, ResolvedContext},
    diff::DeltaRenderer,
    doctor::Doctor,
    domain::{ChangeRequestKey, ProviderKind},
    process::{CommandRunner, TokioCommandRunner},
    providers::{GitHubProvider, GitLabProvider, ProviderError, ProviderRegistry},
    state::{
        ContentIdentity, FileProgress, JsonSessionStore, ReviewSync, SESSION_SCHEMA_VERSION,
        SessionAccess, SessionRestorer, SessionSnapshot, SessionStore, StateError,
    },
    tui::{self, TuiError},
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
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Tui(#[from] TuiError),
    #[error("required dependencies are not ready:\n{0}")]
    Dependencies(String),
    #[error("no review or resumable session was found")]
    NoReview,
}

#[async_trait]
pub trait LaunchBackend: Send + Sync {
    async fn launch(&self, launch: ResolvedLaunch) -> Result<(), LaunchError>;
}

struct InstalledRuntime {
    runner: Arc<TokioCommandRunner>,
    providers: ProviderRegistry,
}

impl InstalledRuntime {
    fn new(runner: Arc<TokioCommandRunner>) -> Self {
        Self {
            providers: ProviderRegistry::new(
                Arc::new(GitHubProvider::new(runner.clone())),
                Arc::new(GitLabProvider::new(runner.clone())),
            ),
            runner,
        }
    }

    async fn launch_key(&self, key: ChangeRequestKey) -> Result<(), LaunchError> {
        let runner: Arc<dyn CommandRunner> = self.runner.clone();
        let provider = self.providers.get(key.provider);
        // The doctor check hits the network (auth status); overlap it with the
        // snapshot load instead of paying for it up front.
        let doctor = Doctor::new(runner);
        let (report, loaded) = tokio::join!(
            doctor.check(Some(key.provider), Some(&key.host)),
            provider.load(&key),
        );
        if !report.is_ready() {
            return Err(LaunchError::Dependencies(report.to_string()));
        }
        let fresh = loaded?;
        let store = JsonSessionStore::discover()?;
        let saved = store.load(&key)?;
        let access = store.open(&key)?;
        let mut snapshot = match saved {
            Some(saved) => SessionRestorer::restore(saved, &fresh).snapshot,
            None => fresh_session(&fresh),
        };
        let (handle, read_only) = match access {
            SessionAccess::ReadWrite(mut handle) => {
                snapshot.updated_at = time::OffsetDateTime::now_utc();
                handle.save(&snapshot)?;
                (Some(handle), false)
            }
            SessionAccess::ReadOnly { .. } => (None, true),
        };
        let renderer = Arc::new(DeltaRenderer::new(self.runner.clone()));
        let runtime = Arc::new(crate::app::Runtime::new(key, provider, renderer, handle));
        let mut app = crate::app::AppState::new(fresh, snapshot);
        if read_only {
            app.notices.push("session is open read-only".into());
        }
        if let Some(file) = app.provider.files.get(app.active_file_index).cloned() {
            let result = runtime
                .execute(crate::app::EffectEnvelope {
                    id: 0,
                    generation: Some(app.provider.head.clone()),
                    effect: crate::app::AppEffect::RenderActiveFile {
                        file,
                        width: app.terminal_width,
                    },
                })
                .await;
            crate::app::update(
                &mut app,
                crate::app::AppEvent::EffectFinished(Box::new(result)),
            );
        }
        let mut terminal = ratatui::init();
        let _restore = TerminalRestore;
        tui::run(&mut terminal, app, runtime).await?;
        Ok(())
    }
}

#[async_trait]
impl LaunchBackend for InstalledRuntime {
    async fn launch(&self, launch: ResolvedLaunch) -> Result<(), LaunchError> {
        match launch {
            ResolvedLaunch::Review(context) => {
                let discovery = context.discovery.ok_or(LaunchError::NoReview)?;
                let kind = match &discovery {
                    crate::context::DiscoveryInput::Exact(key) => key.provider,
                    crate::context::DiscoveryInput::CurrentBranch { provider, .. } => *provider,
                };
                let key = self.providers.get(kind).discover(&discovery).await?;
                self.launch_key(key).await
            }
            ResolvedLaunch::Resume(session_id) => {
                let store = JsonSessionStore::discover()?;
                let sessions = store.list()?;
                let summary = match session_id {
                    Some(id) => sessions.into_iter().find(|summary| {
                        summary.key.session_slug() == id
                            || summary.path.file_stem().and_then(|value| value.to_str())
                                == Some(id.as_str())
                    }),
                    None => sessions.into_iter().next(),
                }
                .ok_or(LaunchError::NoReview)?;
                self.launch_key(summary.key).await
            }
            ResolvedLaunch::Sessions => {
                let store = JsonSessionStore::discover()?;
                for summary in store.list()? {
                    println!(
                        "{}  {:?} {}/{} #{}  {}",
                        summary.key.session_slug(),
                        summary.key.provider,
                        summary.key.host,
                        summary.key.repository,
                        summary.key.number,
                        summary.updated_at
                    );
                }
                Ok(())
            }
        }
    }
}

pub async fn run(cli: Cli) -> Result<(), LaunchError> {
    let concrete = Arc::new(TokioCommandRunner);
    let runner: Arc<dyn CommandRunner> = concrete.clone();
    let runtime = InstalledRuntime::new(concrete);
    run_with(cli, runner, &runtime).await
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

fn fresh_session(snapshot: &crate::domain::ProviderSnapshot) -> SessionSnapshot {
    let files = snapshot
        .files
        .iter()
        .map(|file| {
            let reviewed = file.remotely_reviewed.unwrap_or(false);
            (
                file.path.clone(),
                FileProgress {
                    identity: ContentIdentity {
                        path: file.path.clone(),
                        base_blob: file.base_blob.clone(),
                        head_blob: file.head_blob.clone(),
                    },
                    reviewed,
                    sync: match snapshot.key.provider {
                        ProviderKind::GitHub => ReviewSync::Synced,
                        ProviderKind::GitLab => ReviewSync::LocalOnly,
                    },
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    SessionSnapshot {
        schema_version: SESSION_SCHEMA_VERSION,
        key: snapshot.key.clone(),
        base: snapshot.base.clone(),
        head: snapshot.head.clone(),
        active_file: snapshot.files.first().map(|file| file.path.clone()),
        cursor_row: 0,
        scroll_row: 0,
        files,
        editor: None,
        pending_submit: None,
        updated_at: time::OffsetDateTime::now_utc(),
    }
}

struct TerminalRestore;

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
