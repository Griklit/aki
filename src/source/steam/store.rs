use crate::CONFIG;
use crate::Namespace;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Steam 原始响应的命名空间：数据落在 `database/steam/`。
///
/// 与 Bangumi 存 `Subject` 原样响应同理——接口返回的 JSON 一律先存下来，
/// 解析逻辑改了就离线重跑，不用再打接口。
#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub struct SteamNamespace;

impl Namespace for SteamNamespace {
    fn new() -> Self {
        SteamNamespace
    }

    fn kind() -> &'static str {
        "steam"
    }

    fn data_path() -> PathBuf {
        CONFIG.steam_data_path().to_path_buf()
    }
}

/// 原始响应 + 请求它的 URL。
#[derive(Debug, Clone)]
pub struct SteamRawStore<T> {
    pub url: String,
    pub raw: String,
    _marker: std::marker::PhantomData<T>,
}

impl<T> SteamRawStore<T> {
    #[must_use]
    pub fn new(url: impl Into<String>, raw: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            raw: raw.into(),
            _marker: std::marker::PhantomData,
        }
    }

    /// 写到 `database/steam/{name}.json`。
    pub fn save_named(&self, name: &str) -> Result<PathBuf, std::io::Error> {
        SteamNamespace::save_json(name, self.raw.as_bytes())
    }

    /// 从 `database/steam/{name}.json` 读回原始 JSON 文本。
    pub fn load_named(name: &str) -> Result<String, std::io::Error> {
        std::fs::read_to_string(SteamNamespace::json_path(name))
    }

    /// 从指定路径读回原始 JSON 文本。
    pub fn load_path(path: impl AsRef<std::path::Path>) -> Result<String, std::io::Error> {
        std::fs::read_to_string(path)
    }
}
