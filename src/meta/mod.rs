mod meta_data;
mod meta_id;
mod namespace;

use chrono::Utc;
pub use meta_data::MetaData;
pub use meta_id::MetaId;
pub use namespace::Namespace;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
