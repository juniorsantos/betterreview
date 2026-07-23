use std::sync::Arc;

use crate::domain::ProviderKind;

use super::ReviewProvider;

pub struct ProviderRegistry {
    github: Arc<dyn ReviewProvider>,
    gitlab: Arc<dyn ReviewProvider>,
}

impl ProviderRegistry {
    pub fn new(github: Arc<dyn ReviewProvider>, gitlab: Arc<dyn ReviewProvider>) -> Self {
        Self { github, gitlab }
    }

    pub fn get(&self, kind: ProviderKind) -> Arc<dyn ReviewProvider> {
        match kind {
            ProviderKind::GitHub => self.github.clone(),
            ProviderKind::GitLab => self.gitlab.clone(),
        }
    }
}
