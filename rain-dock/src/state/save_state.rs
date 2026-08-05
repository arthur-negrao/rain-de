use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct PinnedApp {
    pub app_class: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct PinnedJson {
    pub pinned_apps: Vec<PinnedApp>,
}
