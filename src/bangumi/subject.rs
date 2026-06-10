use std::{fs::File, io};

use crate::CONFIG;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    /// Collect
    pub collect: i64,
    /// Doing
    pub doing: i64,
    /// Dropped
    pub dropped: i64,
    /// On Hold
    pub on_hold: i64,
    /// Wish
    pub wish: i64,
}

/// Images
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Images {
    /// Common
    pub common: String,
    /// Grid
    pub grid: String,
    /// Large
    pub large: String,
    /// Medium
    pub medium: String,
    /// Small
    pub small: String,
}

/// Infobox
///
/// Item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    /// Key
    pub key: String,
    /// Value
    pub value: Value,
}

/// Value
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    String(String),
    Array(Vec<ValueArrayItem>),
}

/// KV
///
/// V
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueArrayItem {
    /// K
    pub k: Option<String>,
    /// V
    pub v: String,
}

/// Rating
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rating {
    /// Count
    pub count: Count,
    /// Rank
    pub rank: i64,
    /// Score
    pub score: f64,
    /// Total
    pub total: i64,
}

/// Count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Count {
    #[serde(rename = "1")]
    pub one: Option<i64>,
    #[serde(rename = "2")]
    pub two: Option<i64>,
    #[serde(rename = "3")]
    pub three: Option<i64>,
    #[serde(rename = "4")]
    pub four: Option<i64>,
    #[serde(rename = "5")]
    pub five: Option<i64>,
    #[serde(rename = "6")]
    pub six: Option<i64>,
    #[serde(rename = "7")]
    pub seven: Option<i64>,
    #[serde(rename = "8")]
    pub eight: Option<i64>,
    #[serde(rename = "9")]
    pub nine: Option<i64>,
    #[serde(rename = "10")]
    pub ten: Option<i64>,
}

/// Tags
///
/// Tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    /// Count
    pub count: i64,
    /// Name
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Type {
    Book,
    Anime,
    Music,
    Game,
    Real,
}

impl TryFrom<i64> for Type {
    type Error = crate::utils::error::EnumVariantNotFound;
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Type::Book),
            2 => Ok(Type::Anime),
            3 => Ok(Type::Music),
            4 => Ok(Type::Game),
            6 => Ok(Type::Real),
            _ => Err(crate::utils::error::EnumVariantNotFound),
        }
    }
}

/// Subject
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub id: i64,
    pub r#type: i64,
    pub name: String,
    pub name_cn: String,
    pub summary: String,
    pub series: bool,
    pub nsfw: bool,
    pub locked: bool,
    pub date: Option<NaiveDate>,
    pub platform: String,
    pub images: Images,
    pub infobox: Option<Vec<Item>>,
    pub volumes: i64,
    pub eps: i64,
    pub total_episodes: i64,
    pub rating: Rating,
    pub collection: Collection,
    pub meta_tags: Vec<String>,
    pub tags: Vec<Tag>,
}

impl Subject {
    pub fn load_with_id(id: i64) -> Result<Self, io::Error> {
        let path = CONFIG
            .bangumi_subject_path()
            .join(id.to_string())
            .with_extension("json");
        let file = File::open(path)?;
        let subject: Subject = serde_json::from_reader(file)?;
        Ok(subject)
    }

    pub async fn request_with_id(
        client: &reqwest::Client,
        subject_id: i64,
    ) -> reqwest::Result<Self> {
        let resp = client
            .get(format!(
                "{}/v0/subjects/{}",
                CONFIG.anime.bangumi_api, subject_id
            ))
            .header(reqwest::header::USER_AGENT, "Griklit/aki")
            .send()
            .await?
            .json::<Self>()
            .await?;
        Ok(resp)
    }

    #[inline]
    pub fn r#type_enumed(&self) -> Result<Type, crate::utils::error::EnumVariantNotFound> {
        Type::try_from(self.r#type)
    }
}

#[cfg(test)]
mod tests {
    use super::Subject;

    #[test]
    fn deserialize() {
        let json = include_str!("subject-example.json");
        let _: Subject = serde_json::from_str(json).unwrap();
    }

    #[tokio::test]
    #[ignore = "require-external-internet"]
    async fn request_subject() {
        let ret = Subject::request_with_id(&reqwest::Client::new(), 475354)
            .await
            .unwrap();
        dbg!(ret);
    }
}
