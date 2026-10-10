use crate::Namespace;
use std::path::PathBuf;

/// Bangumi 原始数据的命名空间：`[source.bangumi] path`（默认 `database/source/bangumi`）。
///
/// 一条条目的原始响应落在 `<data_path>/subject/<id>.json`，与 Steam 存原始响应同理
/// ——接口返回的 JSON 先原样存下来，解析逻辑改了就离线重跑，不用再打接口。
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Copy)]
pub struct BangumiNamespace;

impl Namespace for BangumiNamespace {
    fn new() -> Self {
        BangumiNamespace
    }

    fn kind() -> &'static str {
        "bangumi"
    }
}

impl BangumiNamespace {
    /// 单条条目的原始响应路径：`<data_path>/subject/<id>.json`。
    #[must_use]
    pub fn subject_path(subject_id: i64) -> PathBuf {
        Self::file_path(format!("subject/{subject_id}")).with_extension("json")
    }
}
