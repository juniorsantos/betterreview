use std::path::{Path, PathBuf};

use directories::BaseDirs;
use sha2::{Digest, Sha256};

use crate::domain::ChangeRequestKey;

use super::StateError;

#[derive(Debug, Clone)]
pub struct StatePaths {
    root: PathBuf,
}

impl StatePaths {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn discover() -> Result<Self, StateError> {
        if let Some(root) = std::env::var_os("BETTERREVIEW_STATE_DIR") {
            return Ok(Self::new(PathBuf::from(root)));
        }
        let base = BaseDirs::new().ok_or(StateError::StateDirectoryUnavailable)?;
        let root = base
            .state_dir()
            .map(|path| path.join("betterreview"))
            .unwrap_or_else(|| base.data_local_dir().join("betterreview/state"));
        Ok(Self::new(root))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn session_path(&self, key: &ChangeRequestKey) -> Result<PathBuf, StateError> {
        let encoded = serde_json::to_vec(key)?;
        let digest = hex::encode(Sha256::digest(encoded));
        Ok(self.root.join(format!("{digest}.json")))
    }

    pub fn lock_path(&self, key: &ChangeRequestKey) -> Result<PathBuf, StateError> {
        Ok(self.session_path(key)?.with_extension("lock"))
    }
}
