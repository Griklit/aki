use crate::{CONFIG, Namespace, utils};
use std::path::PathBuf;

/// Bangumi 原始数据的命名空间：`[bangumi] path`（默认 `source/bangumi`）。
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Copy)]
pub struct BangumiNamespace;

impl Namespace for BangumiNamespace {
    fn new() -> Self {
        BangumiNamespace
    }

    fn kind() -> &'static str {
        "bangumi"
    }

    fn data_path() -> PathBuf {
        let config = &*CONFIG;
        utils::path::normalize(config.root.join(&CONFIG.bangumi.path))
    }
}

impl BangumiNamespace {
    /// 单条条目的原始响应路径：`<data_path>/subject/<id>.json`。
    #[must_use]
    pub fn subject_path(subject_id: i64) -> PathBuf {
        Self::subject_dir().join(format!("{subject_id}.json"))
    }

    /// 条目原始响应的目录：`<data_path>/subject`。
    #[must_use]
    pub fn subject_dir() -> PathBuf {
        Self::data_path().join("subject")
    }
}
