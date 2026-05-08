//! TOML serde struct definitions for config file parsing.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub(super) struct ConfigFile {
    pub show: Option<ShowConfig>,
    pub filter: Option<FilterConfig>,
    pub order: Option<OrderConfig>,
    pub group: Option<GroupConfig>,
    pub waive: Option<WaiveConfig>,
    pub inject: Option<InjectConfig>,
}

#[derive(Serialize, Deserialize, Default)]
pub(super) struct ShowConfig {
    pub order: Option<Vec<String>>,
    pub time: Option<String>,
    pub shell: Option<bool>,
    pub dir: Option<String>,
    pub repo: Option<bool>,
    pub count: Option<bool>,
    pub exit_code: Option<bool>,
}

#[derive(Serialize, Deserialize, Default)]
pub(super) struct FilterConfig {
    pub this_shell: Option<bool>,
    pub this_dir: Option<bool>,
    pub this_repo: Option<bool>,
    pub today: Option<bool>,
    pub operator: Option<String>,
    pub exit_code: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub(super) struct OrderConfig {
    pub sequence: Option<Vec<String>>,
    pub recency: Option<String>,
    pub frequency: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub(super) struct GroupConfig {
    pub sequence: Option<Vec<String>>,
    pub abspath: Option<bool>,
    pub repo: Option<bool>,
    pub relpath: Option<bool>,
    pub dedup: Option<bool>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub(super) struct WaiveConfig {
    pub commands: Option<Vec<String>>,
    pub min_cmd_len: Option<usize>,
}

#[derive(Serialize, Deserialize, Default)]
pub(super) struct InjectConfig {
    pub bash: Option<String>,
    pub zsh: Option<String>,
    pub tcsh: Option<String>,
}
