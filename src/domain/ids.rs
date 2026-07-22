use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    GitHub,
    GitLab,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChangeRequestKey {
    pub provider: ProviderKind,
    pub host: String,
    pub repository: String,
    pub number: u64,
}

impl ChangeRequestKey {
    pub fn session_slug(&self) -> String {
        fn clean(value: &str, preserve_dots: bool) -> String {
            value
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || (preserve_dots && ch == '.') {
                        ch
                    } else {
                        '-'
                    }
                })
                .collect()
        }

        format!(
            "{}-{}-{}-{}",
            match self.provider {
                ProviderKind::GitHub => "github",
                ProviderKind::GitLab => "gitlab",
            },
            clean(&self.host, true),
            clean(&self.repository, false),
            self.number,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommitOid(pub String);

impl AsRef<str> for CommitOid {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RepoPath(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DraftId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadId(pub String);
