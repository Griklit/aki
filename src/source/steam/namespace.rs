use crate::{CONFIG, Namespace, utils};
use std::path::PathBuf;

/// Steam 原始响应的命名空间：`[steam] path`（默认 `source/steam`）。
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Copy)]
pub struct SteamNamespace;

impl Namespace for SteamNamespace {
    fn new() -> Self {
        SteamNamespace
    }

    fn kind() -> &'static str {
        "steam"
    }

    fn data_path() -> PathBuf {
        let config = &*CONFIG;
        utils::path::normalize(config.root.join(&CONFIG.steam.path))
    }
}

impl SteamNamespace {
    /// 某个存放名对应的原始响应文件：`<data_path>/<存放名>.json`。
    #[must_use]
    pub fn saved_path(name: &str) -> PathBuf {
        Self::data_path().join(name).with_extension("json")
    }
}
