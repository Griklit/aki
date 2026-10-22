use super::namespace::{Namespace, deserialize_namespace, serialize_namespace};
use crate::utils;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MetaId<T: Namespace> {
    #[serde(
        serialize_with = "serialize_namespace",
        deserialize_with = "deserialize_namespace"
    )]
    namespace: T,
    pub id: String,
}

impl<T: Namespace> MetaId<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            namespace: T::new(),
            id: uuid::Uuid::now_v7().to_string(),
        }
    }

    #[must_use]
    pub fn with_id(id: String) -> Self {
        Self {
            namespace: T::new(),
            id,
        }
    }

    /// 文件名（含扩展名）：`id` 经文件名安全化后加 `.json`。
    #[must_use]
    pub fn json_filename(&self) -> String {
        format!("{}.json", utils::path::sanitize_name(&self.id))
    }

    /// 数据文件路径：`{T::data_path()}/{安全化后的 id}.json`。
    #[must_use]
    pub fn json_data_path(&self) -> PathBuf {
        T::data_path().join(self.json_filename())
    }
}

impl<T: Namespace> Default for MetaId<T> {
    fn default() -> Self {
        Self::new()
    }
}
