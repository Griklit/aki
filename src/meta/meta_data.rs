use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MetaData {
    pub title: Option<String>,
    pub create_time: Option<DateTime<Utc>>,
}
