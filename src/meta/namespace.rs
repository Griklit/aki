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
/// 只定义三件事：[`Self::kind`]、[`Self::root_path`]、[`Self::data_path`]。
pub trait Namespace {
    fn new() -> Self;

    /// 命名空间标识，如 `"anime"`。
    fn kind() -> &'static str;

    /// 数据根目录（`Config::root`），即 `aki.toml` 所在目录。
    #[must_use]
    fn root_path() -> PathBuf {
        crate::CONFIG.root.clone()
    }

    /// 该命名空间的数据根目录，例如 `database/anime`。
    #[must_use]
    fn data_path() -> PathBuf;
}
