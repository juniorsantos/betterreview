use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    diff::{DiffRenderer, parse_file_patch},
    domain::ChangeRequestKey,
    providers::ReviewProvider,
    state::SessionHandle,
};

use super::{AppEffect, EffectEnvelope, EffectOutcome, EffectResult, RenderedFile};

#[derive(Clone)]
pub struct Runtime {
    key: ChangeRequestKey,
    provider: Arc<dyn ReviewProvider>,
    renderer: Arc<dyn DiffRenderer>,
    runner: Arc<dyn crate::process::CommandRunner>,
    session: Option<Arc<Mutex<SessionHandle>>>,
}

impl Runtime {
    pub fn new(
        key: ChangeRequestKey,
        provider: Arc<dyn ReviewProvider>,
        renderer: Arc<dyn DiffRenderer>,
        runner: Arc<dyn crate::process::CommandRunner>,
        session: Option<SessionHandle>,
    ) -> Self {
        Self {
            key,
            provider,
            renderer,
            runner,
            session: session.map(|handle| Arc::new(Mutex::new(handle))),
        }
    }

    pub async fn execute(&self, envelope: EffectEnvelope) -> EffectResult {
        let outcome = match envelope.effect {
            AppEffect::RenderActiveFile { file, width } => {
                let result = async {
                    let parsed = parse_file_patch(
                        &file,
                        envelope.generation.as_ref().ok_or("render has no head")?,
                    )
                    .map_err(|error| error.to_string())?;
                    let patch = match &file.patch {
                        crate::domain::PatchAvailability::Available(patch) => patch.as_bytes(),
                        _ => return Err("patch is unavailable".into()),
                    };
                    let rendered = self
                        .renderer
                        .render(patch, &parsed, width)
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok(RenderedFile { parsed, rendered })
                }
                .await;
                EffectOutcome::Rendered(result)
            }
            AppEffect::SaveConfig { config } => {
                let result = crate::state::StatePaths::discover()
                    .and_then(|paths| config.save(&paths))
                    .map_err(|error| error.to_string());
                EffectOutcome::Saved(result)
            }
            AppEffect::SaveSession { snapshot } => {
                let result = match &self.session {
                    Some(handle) => handle
                        .lock()
                        .await
                        .save(&snapshot)
                        .map_err(|error| error.to_string()),
                    None => Err("session is read-only".into()),
                };
                EffectOutcome::Saved(result)
            }
            AppEffect::CreateDraft { input } => {
                let result = match envelope.generation.as_ref() {
                    Some(head) => self
                        .provider
                        .create_draft(&self.key, head, input)
                        .await
                        .map_err(|error| error.to_string()),
                    None => Err("draft operation has no head generation".into()),
                };
                EffectOutcome::DraftCreated(result)
            }
            AppEffect::UpdateDraft { id, body } => EffectOutcome::DraftUpdated(
                self.provider
                    .update_draft(&self.key, &id, body)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            AppEffect::DeleteDraft { id } => {
                let result = self
                    .provider
                    .delete_draft(&self.key, &id)
                    .await
                    .map_err(|error| error.to_string());
                EffectOutcome::DraftDeleted { id, result }
            }
            AppEffect::Reply { thread, body } => EffectOutcome::ThreadUpdated(
                self.provider
                    .reply(&self.key, &thread, body)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            AppEffect::ResolveThread { thread, resolved } => EffectOutcome::Completed(
                self.provider
                    .resolve_thread(&self.key, &thread, resolved)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            AppEffect::SetFileReviewed { path, reviewed } => EffectOutcome::FileReviewed {
                path: path.clone(),
                reviewed,
                result: self
                    .provider
                    .set_file_reviewed(&self.key, &path, reviewed)
                    .await
                    .map_err(|error| error.to_string()),
            },
            AppEffect::RefreshSnapshot => EffectOutcome::SnapshotRefreshed(Box::new(
                self.provider
                    .load(&self.key)
                    .await
                    .map_err(|error| error.to_string()),
            )),
            AppEffect::SubmitReview { request } => {
                let result = match self.provider.read_head(&self.key).await {
                    Ok(head) if head == request.expected_head => self
                        .provider
                        .submit_review(&self.key, request)
                        .await
                        .map_err(|error| error.to_string()),
                    Ok(head) => Err(format!(
                        "review head changed from {} to {}; refresh before submitting",
                        request.expected_head.0, head.0
                    )),
                    Err(error) => Err(error.to_string()),
                };
                EffectOutcome::ReviewSubmitted(result)
            }
            AppEffect::DiscardReview => EffectOutcome::Completed(
                self.provider
                    .discard_review(&self.key)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            AppEffect::LoadBlame { path, revision } => EffectOutcome::BlameLoaded {
                path: path.clone(),
                result: crate::blame::load(self.runner.as_ref(), &path, &revision).await,
            },
            AppEffect::LoadFileContext { path, revision } => EffectOutcome::FileContextLoaded {
                path: path.clone(),
                result: self
                    .provider
                    .read_file(&self.key, &path, &revision)
                    .await
                    .map_err(|error| error.to_string()),
            },
            AppEffect::CopyToClipboard { content } => EffectOutcome::ClipboardCopied(
                crate::clipboard::copy(&content).map_err(|error| error.to_string()),
            ),
            AppEffect::OpenLink { url } => {
                EffectOutcome::LinkOpened(crate::browser::open(self.runner.as_ref(), &url).await)
            }
        };
        EffectResult {
            id: envelope.id,
            generation: envelope.generation,
            outcome,
        }
    }
}
