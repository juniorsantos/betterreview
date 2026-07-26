use std::{
    fs, io,
    path::{Path, PathBuf},
};

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
    pub fn load(state: &StatePaths) -> Self {
        if let Some(config) = Self::read(&Self::path()) {
            return config;
        }
        Self::read(&legacy_path(state)).unwrap_or_default()
    }

    pub fn save(&self, _state: &StatePaths) -> Result<(), StateError> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn path() -> PathBuf {
        if let Some(dir) = std::env::var_os("BETTERREVIEW_CONFIG_DIR") {
            return PathBuf::from(dir).join("config.json");
        }
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".config")))
            .unwrap_or_else(|| PathBuf::from(".config"));
        base.join("betterreview").join("config.json")
    }

    fn read(path: &Path) -> Option<Self> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).ok(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(_) => None,
        }
    }
}

fn legacy_path(state: &StatePaths) -> PathBuf {
    state.root().join("config.json")
}
