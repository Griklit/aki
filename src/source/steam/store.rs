use super::SteamNamespace;
use crate::Namespace;

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

    /// 写到 `SteamNamespace` 的 `{name}.json`。
    ///
    /// 写入的是 [`Self::raw`] 本身——响应原文，一个字节都不改。
    pub fn save_named(&self, name: &str) -> Result<std::path::PathBuf, std::io::Error> {
        SteamNamespace::save_json(name, self.raw.as_bytes())
    }

    /// 从 `SteamNamespace` 的 `{name}.json` 读回原始 JSON 文本。
    pub fn load_named(name: &str) -> Result<String, std::io::Error> {
        std::fs::read_to_string(SteamNamespace::json_path(name))
    }

    /// 从指定路径读回原始 JSON 文本。
    pub fn load_path(path: impl AsRef<std::path::Path>) -> Result<String, std::io::Error> {
        std::fs::read_to_string(path)
    }
}
