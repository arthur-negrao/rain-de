use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct PinnedApp(pub String);

impl PinnedApp {
    pub fn app_class(&self) -> &str {
        self.as_str()
    }
}

impl Deref for PinnedApp {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PinnedApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct PinnedJson(pub Vec<PinnedApp>);

impl PinnedJson {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

impl Deref for PinnedJson {
    type Target = Vec<PinnedApp>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PinnedJson {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
