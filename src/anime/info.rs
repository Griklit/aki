// use super::{Resource, resource_type};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnimeInfo {
    pub title: String,
    pub summary: Option<String>,
    pub platform: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub season: Option<u16>,
    pub episodes: Option<u16>,
    #[serde(default)]
    pub tags: Vec<String>,
    // pub cover: Option<Resource<resource_type::Cover>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnimeInfoInternationalization {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl AnimeInfoInternationalization {
    #[must_use]
    pub const fn new(title: Option<String>, summary: Option<String>) -> Self {
        Self { title, summary }
    }
}
