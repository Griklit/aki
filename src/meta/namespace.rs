use std::fs;
use std::io;
use std::path::PathBuf;

use serde::Deserialize;

use serde::{Deserializer, Serializer};

pub(super) fn serialize_namespace<T, S>(_: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Namespace,
    S: Serializer,
{
    serializer.serialize_str(T::kind())
}

pub(super) fn deserialize_namespace<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Namespace,
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let namespace_kind = T::kind();
    if s != namespace_kind {
        return Err(serde::de::Error::custom(format!(
            "namespace mismatch: {namespace_kind} required, but {s} found",
        )));
    }
    Ok(T::new())
}

/// 命名空间：一个领域的数据全部落在同一个目录下，用 `kind()` 区分。
///
/// 路径相关的静态方法由各命名空间自己提供，[`crate::MetaId`] 因此不再硬编码
/// 任何具体领域的目录（新增一个领域时只需要实现 [`Self::data_path`]）。
pub trait Namespace {
    fn new() -> Self;

    /// 命名空间标识，同时是 [`crate::MetaId`] 里的序列化值（如 `"anime"`）。
    fn kind() -> &'static str;

    /// 该命名空间的数据根目录，例如 `database/anime`。
    ///
    /// 默认实现从 [`crate::CONFIG`] 按 `kind()` 取路径；某个领域的目录不由配置
    /// 决定时，覆盖它即可。
    fn data_path() -> PathBuf {
        crate::CONFIG.namespace_data_path(Self::kind())
    }

    /// 单条数据的 JSON 文件路径：`{data_path}/{name}.json`（目录会按需创建）。
    #[must_use]
    fn json_path(name: &str) -> PathBuf {
        let path = sanitize_filename::sanitize(name);
        Self::data_path().join(path).with_extension("json")
    }

    /// 把 `bytes` 写到 `{data_path}/{name}.json`，覆盖已有文件。
    fn save_json(name: &str, bytes: &[u8]) -> Result<PathBuf, io::Error> {
        let path = Self::json_path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, bytes)?;
        Ok(path)
    }
}
