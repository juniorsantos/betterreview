use serde::Deserialize;

#[derive(Deserialize)]
pub struct GraphQlEnvelope<T> {
    pub data: Option<T>,
    #[serde(default)]
    pub errors: Vec<GraphQlError>,
}

#[derive(Deserialize)]
pub struct GraphQlError {
    pub message: String,
}

#[derive(Deserialize)]
pub struct SnapshotData {
    pub viewer: Option<Author>,
    pub repository: Option<SnapshotRepository>,
}

#[derive(Deserialize)]
pub struct SnapshotRepository {
    #[serde(rename = "pullRequest")]
    pub pull_request: Option<PullRequest>,
}

#[derive(Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub author: Option<Author>,
    #[serde(rename = "baseRefOid")]
    pub base_ref_oid: String,
    #[serde(rename = "headRefOid")]
    pub head_ref_oid: String,
    #[serde(default)]
    pub files: NodeConnection<ViewedFile>,
    #[serde(rename = "reviewThreads", default)]
    pub review_threads: NodeConnection<ReviewThread>,
}

#[derive(Deserialize)]
pub struct Author {
    pub login: String,
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct NodeConnection<T> {
    #[serde(default)]
    pub nodes: Vec<T>,
    #[serde(rename = "pageInfo", default)]
    pub page_info: PageInfo,
}

impl<T> Default for NodeConnection<T> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            page_info: PageInfo::default(),
        }
    }
}

#[derive(Default, Deserialize)]
pub struct PageInfo {
    #[serde(rename = "hasNextPage", default)]
    pub has_next_page: bool,
    #[serde(rename = "endCursor")]
    pub end_cursor: Option<String>,
}

#[derive(Deserialize)]
pub struct ViewedFile {
    pub path: String,
    #[serde(rename = "viewerViewedState")]
    pub viewer_viewed_state: Option<String>,
}

#[derive(Deserialize)]
pub struct ReviewThread {
    pub id: String,
    pub path: String,
    #[serde(rename = "diffSide")]
    pub diff_side: String,
    #[serde(rename = "isResolved")]
    pub is_resolved: bool,
    #[serde(rename = "isOutdated")]
    pub is_outdated: bool,
    #[serde(default)]
    pub comments: NodeConnection<ReviewComment>,
}

#[derive(Deserialize)]
pub struct ReviewComment {
    pub id: String,
    pub body: String,
    pub author: Option<Author>,
    pub line: Option<u32>,
    #[serde(rename = "originalLine")]
    pub original_line: Option<u32>,
    #[serde(rename = "startLine")]
    pub start_line: Option<u32>,
    #[serde(rename = "originalStartLine")]
    pub original_start_line: Option<u32>,
    #[serde(rename = "viewerDidAuthor", default)]
    pub viewer_did_author: bool,
    #[serde(rename = "pullRequestReview")]
    pub pull_request_review: Option<ReviewState>,
}

#[derive(Deserialize)]
pub struct ReviewState {
    pub state: String,
}

#[derive(Deserialize)]
pub struct ListData {
    pub repository: Option<ListRepository>,
}

#[derive(Deserialize)]
pub struct ListRepository {
    #[serde(rename = "pullRequests")]
    pub pull_requests: ListConnection,
}

#[derive(Deserialize)]
pub struct ListConnection {
    pub nodes: Vec<ListNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNode {
    pub number: u64,
    pub title: String,
    pub is_draft: bool,
    pub updated_at: String,
    pub head_ref_name: String,
    pub url: String,
    pub author: Option<Author>,
    pub body: Option<String>,
}

#[derive(Deserialize)]
pub struct RestFile {
    pub filename: String,
    pub previous_filename: Option<String>,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub patch: Option<String>,
    pub sha: Option<String>,
}
