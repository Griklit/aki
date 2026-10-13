use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnimeInternationalization {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl AnimeInternationalization {
    #[must_use]
    pub const fn new(title: Option<String>, summary: Option<String>) -> Self {
        Self { title, summary }
    }
}
