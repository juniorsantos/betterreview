use sha1::Digest as _;
use url::Url;

use crate::domain::{ChangeRequestKey, CommitOid, ProviderKind, RepoPath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewLinks {
    provider: ProviderKind,
    review: Url,
    repository: Url,
}

impl ReviewLinks {
    pub fn new(key: &ChangeRequestKey, web_url: &str) -> Option<Self> {
        let mut review = Url::parse(web_url).ok()?;
        if !matches!(review.scheme(), "http" | "https")
            || review.host_str()? != key.host
            || review.path().trim_end_matches('/') != review_path(key)
        {
            return None;
        }
        review.set_query(None);
        review.set_fragment(None);
        review.set_path(&review_path(key));

        let mut repository = review.clone();
        repository.set_path(&format!("/{}", key.repository));

        Some(Self {
            provider: key.provider,
            review,
            repository,
        })
    }

    pub fn review_url(&self) -> &str {
        self.review.as_str()
    }

    pub fn commit_url(&self, commit: &CommitOid) -> String {
        let mut url = self.repository.clone();
        let mut segments = url
            .path_segments_mut()
            .expect("HTTP review URLs are hierarchical");
        match self.provider {
            ProviderKind::GitHub => {
                segments.push("commit");
            }
            ProviderKind::GitLab => {
                segments.push("-").push("commit");
            }
        }
        segments.push(commit.as_ref());
        drop(segments);
        url.into()
    }

    pub fn file_url(&self, path: &RepoPath) -> String {
        let mut url = self.review.clone();
        let mut segments = url
            .path_segments_mut()
            .expect("HTTP review URLs are hierarchical");
        match self.provider {
            ProviderKind::GitHub => {
                segments.push("files");
            }
            ProviderKind::GitLab => {
                segments.push("diffs");
            }
        }
        drop(segments);
        let hash = match self.provider {
            ProviderKind::GitHub => format!("{:x}", sha2::Sha256::digest(path.0.as_bytes())),
            ProviderKind::GitLab => format!("{:x}", sha1::Sha1::digest(path.0.as_bytes())),
        };
        url.set_fragment(Some(&match self.provider {
            ProviderKind::GitHub => format!("diff-{hash}"),
            ProviderKind::GitLab => hash,
        }));
        url.into()
    }
}

fn review_path(key: &ChangeRequestKey) -> String {
    match key.provider {
        ProviderKind::GitHub => format!("/{}/pull/{}", key.repository, key.number),
        ProviderKind::GitLab => format!("/{}/-/merge_requests/{}", key.repository, key.number),
    }
}
