use ansi_to_tui::IntoText as _;
use async_trait::async_trait;
use ratatui::text::Line;
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, sync::Arc, time::Duration};

use crate::{
    domain::DiffPosition,
    process::{CommandError, CommandRunner, CommandSpec},
};

use super::{ParsedFileDiff, sanitize_ansi};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowBinding {
    pub row_index: usize,
    pub left: Option<DiffPosition>,
    pub right: Option<DiffPosition>,
}

#[derive(Debug, Clone)]
pub struct RenderedRow {
    pub text: Line<'static>,
    pub binding: RowBinding,
}

#[derive(Debug, Clone)]
pub struct RenderedDiff {
    pub rows: Vec<RenderedRow>,
}

#[derive(Debug, thiserror::Error)]
pub enum DeltaError {
    #[error("delta executable was not found; install git-delta and retry")]
    Missing,
    #[error("delta timed out after {0:?}")]
    Timeout(Duration),
    #[error("delta exited with status {status}: {stderr}")]
    Failed { status: i32, stderr: String },
    #[error("delta changed patch structure: expected {expected} rows, received {actual}")]
    StructureChanged { expected: usize, actual: usize },
    #[error("delta output was not valid UTF-8")]
    InvalidUtf8,
    #[error("ANSI styling could not be converted: {0}")]
    Ansi(String),
}

#[async_trait]
pub trait DiffRenderer: Send + Sync {
    async fn render(
        &self,
        patch: &[u8],
        parsed: &ParsedFileDiff,
        width: u16,
    ) -> Result<RenderedDiff, DeltaError>;
}

pub struct DeltaRenderer {
    runner: Arc<dyn CommandRunner>,
}

impl DeltaRenderer {
    pub fn new(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl DiffRenderer for DeltaRenderer {
    async fn render(
        &self,
        patch: &[u8],
        parsed: &ParsedFileDiff,
        width: u16,
    ) -> Result<RenderedDiff, DeltaError> {
        let _cache_key = (&parsed.head, width);
        let timeout = Duration::from_secs(60);
        let output = self
            .runner
            .run(CommandSpec {
                program: PathBuf::from("delta"),
                args: [
                    "--paging=never".to_owned(),
                    "--color-only".to_owned(),
                    "--detect-dark-light=never".to_owned(),
                    "--max-line-length=0".to_owned(),
                    "--true-color=always".to_owned(),
                    format!("--plus-style=syntax {}", crate::tui::theme::DELTA_PLUS_BG),
                    format!("--minus-style=syntax {}", crate::tui::theme::DELTA_MINUS_BG),
                    format!(
                        "--plus-emph-style=syntax {}",
                        crate::tui::theme::DELTA_PLUS_EMPH_BG
                    ),
                    format!(
                        "--minus-emph-style=syntax {}",
                        crate::tui::theme::DELTA_MINUS_EMPH_BG
                    ),
                ]
                .into_iter()
                .map(OsString::from)
                .collect(),
                stdin: Some(patch.to_vec()),
                cwd: None,
                timeout,
                env: BTreeMap::new(),
                env_remove: Vec::new(),
            })
            .await
            .map_err(|error| map_command_error(error, timeout))?;
        if output.status != 0 {
            return Err(DeltaError::Failed {
                status: output.status,
                stderr: redact_stderr(&output.stderr),
            });
        }

        let sanitized = sanitize_ansi(&output.stdout)?;
        let output_rows = split_lines(&sanitized);
        if output_rows.len() != parsed.rows.len() {
            return Err(DeltaError::StructureChanged {
                expected: parsed.rows.len(),
                actual: output_rows.len(),
            });
        }

        let rows = output_rows
            .into_iter()
            .enumerate()
            .map(|(index, bytes)| {
                let text = bytes
                    .into_text()
                    .map_err(|error| DeltaError::Ansi(error.to_string()))?;
                let line = match text.lines.as_slice() {
                    [] => Line::default(),
                    [line] => line.clone(),
                    _ => {
                        return Err(DeltaError::StructureChanged {
                            expected: 1,
                            actual: text.lines.len(),
                        });
                    }
                };
                Ok(RenderedRow {
                    text: line,
                    binding: RowBinding {
                        row_index: index,
                        left: parsed.rows[index].left.clone(),
                        right: parsed.rows[index].right.clone(),
                    },
                })
            })
            .collect::<Result<Vec<_>, DeltaError>>()?;

        Ok(RenderedDiff { rows })
    }
}

fn split_lines(bytes: &[u8]) -> Vec<&[u8]> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let mut rows: Vec<_> = bytes.split(|byte| *byte == b'\n').collect();
    if bytes.ends_with(b"\n") {
        rows.pop();
    }
    for row in &mut rows {
        if row.ends_with(b"\r") {
            *row = &row[..row.len() - 1];
        }
    }
    rows
}

fn map_command_error(error: CommandError, timeout: Duration) -> DeltaError {
    match error {
        CommandError::Timeout { .. } => DeltaError::Timeout(timeout),
        CommandError::Spawn { .. } => DeltaError::Missing,
        CommandError::Io(error) => DeltaError::Failed {
            status: -1,
            stderr: error.to_string(),
        },
    }
}

fn redact_stderr(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let had_trailing_newline = text.ends_with('\n');
    let mut redacted = text
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if ["token", "authorization", "private-token", "oauth"]
                .iter()
                .any(|pattern| lower.contains(pattern))
            {
                "[REDACTED]".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if had_trailing_newline {
        redacted.push('\n');
    }
    redacted
}
