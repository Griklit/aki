use super::SteamNamespace;

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

    /// 写到 [`SteamNamespace::saved_path`] 给出的 `{name}.json`。
    ///
    /// 写入的是 [`Self::raw`] 本身——响应原文，一个字节都不改。
    ///
    /// # Errors
    /// 目录建不出来或写盘失败时返回错误。
    pub fn save_named(&self, name: &str) -> Result<std::path::PathBuf, std::io::Error> {
        let path = SteamNamespace::saved_path(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, self.raw.as_bytes())?;
        Ok(path)
    }

    /// 从 [`SteamNamespace::saved_path`] 给出的 `{name}.json` 读回原始 JSON 文本。
    ///
    /// # Errors
    /// 文件不存在或读取失败时返回错误。
    pub fn load_named(name: &str) -> Result<String, std::io::Error> {
        std::fs::read_to_string(SteamNamespace::saved_path(name))
    }

    /// 从指定路径读回原始 JSON 文本。
    ///
    /// # Errors
    /// 文件不存在或读取失败时返回错误。
    pub fn load_path(path: impl AsRef<std::path::Path>) -> Result<String, std::io::Error> {
        std::fs::read_to_string(path)
    }
}
