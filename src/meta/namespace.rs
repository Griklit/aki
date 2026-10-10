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
/// trait 本身只说三件事：**是哪个命名空间**（[`Self::kind`]）、**数据根目录在哪**
/// （[`Self::root_path`]）、**这个命名空间的数据放在哪**（[`Self::data_path`]）。
///
/// 具体的文件名／子目录（`subject/{id}.json`、`appdetails/{appid}.json`、索引目录…）
/// 由**各个命名空间的固有方法**提供，因为它们各自规则不同；trait 上不放
/// `json_path()` / `save_json()` 这类通用文件操作——那既绑死了「平铺一个 json」的
/// 假设，又把读写职责混进了路径抽象里。
///
/// [`crate::MetaId`] 是唯一按 `kind()` 泛型使用它的地方（`MetaId<T>` 靠
/// `data_path()` 推自己的文件名），其余调用方都直接用具体命名空间的固有方法。
pub trait Namespace {
    fn new() -> Self;

    /// 命名空间标识，同时是 [`crate::MetaId`] 里的序列化值（如 `"anime"`）。
    fn kind() -> &'static str;

    /// 数据根目录（`Config::root`），即 `aki.toml` 所在目录。
    #[must_use]
    fn root_path() -> PathBuf {
        crate::CONFIG.root.clone()
    }

    /// 该命名空间的数据根目录，例如 `database/anime`。
    ///
    /// 每个命名空间都自己实现：用 [`Self::root_path`] 拼上配置里自己那段相对路径。
    #[must_use]
    fn data_path() -> PathBuf;
}
