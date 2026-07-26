use serde::Deserialize;

#[derive(Deserialize)]
pub struct MergeRequest {
    pub iid: u64,
    pub title: String,
    pub web_url: String,
    pub author: Author,
    pub diff_refs: DiffRefs,
}

#[derive(Clone, Deserialize)]
pub struct DiffRefs {
    pub base_sha: String,
    pub start_sha: String,
    pub head_sha: String,
}

#[derive(Deserialize)]
pub struct Author {
    pub username: String,
}

#[derive(Deserialize)]
pub struct Changes {
    pub changes: Vec<Diff>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Diff {
    pub old_path: String,
    pub new_path: String,
    #[serde(default)]
    pub new_file: bool,
    #[serde(default)]
    pub deleted_file: bool,
    #[serde(default)]
    pub renamed_file: bool,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub too_large: bool,
    pub diff: Option<String>,
}

#[derive(Deserialize)]
pub struct DraftNote {
    pub id: u64,
    pub note: String,
    pub position: Option<Position>,
}

#[derive(Clone, Deserialize)]
pub struct Position {
    pub old_path: String,
    pub new_path: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

#[derive(Deserialize)]
pub struct Discussion {
    pub id: String,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default)]
    pub notes: Vec<Note>,
}

#[derive(Deserialize)]
pub struct Note {
    pub id: u64,
    pub author: Author,
    pub body: String,
    #[serde(default)]
    pub resolved: bool,
    pub position: Option<Position>,
}

#[derive(Deserialize)]
pub struct Approvals {
    #[serde(rename = "approved")]
    pub _approved: bool,
    #[serde(rename = "approvals_required", default)]
    pub _approvals_required: u32,
}

#[derive(Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub tier: Option<String>,
}

#[derive(Deserialize)]
pub struct Blob {
    pub blob_id: String,
}

#[derive(Deserialize)]
pub struct MergeRequestSummary {
    pub iid: u64,
    pub title: String,
    pub draft: bool,
    pub updated_at: String,
    pub source_branch: String,
    pub web_url: String,
    pub author: Author,
    pub description: Option<String>,
}
