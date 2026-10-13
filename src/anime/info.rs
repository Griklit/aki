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

impl AnimeInfo {
    #[must_use]
    pub fn air_date_formatter(date: &NaiveDate) -> String {
        date.to_string()
    }

    #[must_use]
    pub fn air_date_string(&self) -> Option<String> {
        self.air_date.map(|date| Self::air_date_formatter(&date))
    }
}
