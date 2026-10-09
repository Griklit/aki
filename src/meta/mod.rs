mod namespace;

use chrono::{DateTime, Utc};
pub use namespace::Namespace;
use namespace::{deserialize_namespace, serialize_namespace};
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

    /// 文件名（含扩展名），由 `id` 经文件名安全化得到。
    #[must_use]
    pub fn json_filename(&self) -> String {
        T::json_path(&self.id).file_name().map_or_else(
            || String::from("unnamed.json"),
            |v| v.to_string_lossy().into(),
        )
    }

    /// 数据文件路径，目录与命名规则均由命名空间 `T` 决定。
    #[must_use]
    pub fn json_data_path(&self) -> PathBuf {
        T::json_path(&self.id)
    }
}

impl<T: Namespace> Default for MetaId<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MetaData {
    pub title: Option<String>,
    pub create_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Meta<T: Namespace> {
    pub id: MetaId<T>,
    pub data: MetaData,
}

impl<T: Namespace> Meta<T> {
    #[must_use]
    pub fn new(title: Option<String>) -> Self {
        Self {
            id: MetaId::new(),
            data: MetaData {
                title,
                create_time: Some(Utc::now()),
            },
        }
    }

    #[must_use]
    pub fn with_id(id: String, title: Option<String>) -> Self {
        Self {
            id: MetaId::with_id(id),
            data: MetaData {
                title,
                create_time: Some(Utc::now()),
            },
        }
    }

    pub fn json_data_path(&self) -> PathBuf {
        self.id.json_data_path()
    }
}
