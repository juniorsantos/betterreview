use std::{fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::DiffLayout;

use super::{StateError, StatePaths};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub diff_layout: DiffLayout,
    #[serde(default)]
    pub files_hidden: bool,
}

impl AppConfig {
    pub fn load(paths: &StatePaths) -> Self {
        Self::read(&Self::path(paths)).unwrap_or_default()
    }

    pub fn save(&self, paths: &StatePaths) -> Result<(), StateError> {
        let path = Self::path(paths);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    fn path(paths: &StatePaths) -> PathBuf {
        paths.root().join("config.json")
    }

    fn read(path: &PathBuf) -> Option<Self> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).ok(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(_) => None,
        }
    }
}
