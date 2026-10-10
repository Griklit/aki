use crate::{CONFIG, Namespace, utils};
use std::path::PathBuf;

/// Steam 原始响应的命名空间：`[steam] path`（默认 `database/source/steam`）。
///
/// 与 Bangumi 存 `Subject` 原样响应同理——接口返回的 JSON 一律先存下来，
/// 解析逻辑改了就离线重跑，不用再打接口。
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
    ///
    /// 存放名是 Steam 自己的约定（`appdetails/{appid}`、`owned-games`），
    /// 所以由这里给出，而不是 trait 上的通用 `json_path()`。
    #[must_use]
    pub fn saved_path(name: &str) -> PathBuf {
        Self::data_path().join(name).with_extension("json")
    }
}
