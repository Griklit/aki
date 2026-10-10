use std::{fs::File, io};

use super::BangumiNamespace;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl Type {
    /// 接口里表示这个类型的数字，与 [`TryFrom<i64>`] 互为反向。
    #[must_use]
    pub fn code(self) -> i64 {
        match self {
            Type::Book => 1,
            Type::Anime => 2,
            Type::Music => 3,
            Type::Game => 4,
            Type::Real => 6,
        }
    }

    /// 用于展示的小写名字，与接口文档里的枚举名一致。
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Type::Book => "book",
            Type::Anime => "anime",
            Type::Music => "music",
            Type::Game => "game",
            Type::Real => "real",
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
    /// 播出/发售日期。接口在日期未定时给空字符串，按「没有日期」处理。
    #[serde(default, deserialize_with = "deserialize_date")]
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

/// 容错地读 `date` 字段：空字符串（日期未定）等价于没有日期。
fn deserialize_date<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    match raw.as_deref() {
        None | Some("") => Ok(None),
        Some(text) => NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

impl Subject {
    pub fn save(&self) -> Result<(), io::Error> {
        let path = BangumiNamespace::subject_path(self.id);
        let file = File::create(path)?;
        let writer = io::BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;
        Ok(())
    }

    pub fn load_with_id(id: i64) -> Result<Self, io::Error> {
        let path = BangumiNamespace::subject_path(id);
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
                CONFIG.bangumi.bangumi_api, subject_id
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

    /// 按语言挑一个名字：中文环境优先用 `name_cn`，其余语言用原名。
    ///
    /// 接口没有英文名字段，所以 `EnUs` 也只会拿到原名。`name_cn` 为空时同样退回原名，
    /// 因此结果**保证非空**（原名是必填字段）。
    #[must_use]
    pub fn localized_name(&self, language: crate::Language) -> &str {
        match language {
            crate::Language::ZhCn if !self.name_cn.is_empty() => &self.name_cn,
            _ => &self.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Subject, Type};

    #[test]
    fn deserialize() {
        let json = include_str!("subject-example.json");
        let _: Subject = serde_json::from_str(json).unwrap();
    }

    /// 搜索接口与条目接口在日期未定时给的是空字符串，不能当成解析失败。
    #[test]
    fn empty_date_is_no_date() {
        let mut value: serde_json::Value =
            serde_json::from_str(include_str!("subject-example.json")).unwrap();
        assert!(value["date"].is_string(), "样本里应当有 date 字段");
        value["date"] = serde_json::Value::String(String::new());
        let subject: Subject = serde_json::from_value(value).unwrap();
        assert!(subject.date.is_none());
    }

    /// `code()` 与 `TryFrom<i64>` 必须互为反向，否则筛选条件和展示会对不上。
    #[test]
    fn type_code_round_trips() {
        for subject_type in [
            Type::Book,
            Type::Anime,
            Type::Music,
            Type::Game,
            Type::Real,
        ] {
            assert_eq!(Type::try_from(subject_type.code()).unwrap(), subject_type);
            assert!(!subject_type.as_str().is_empty());
        }
        assert!(Type::try_from(5).is_err(), "5 不是合法的条目类型");
    }

    /// 中文环境用 `name_cn`，其他语言与 `name_cn` 为空时都用原名。
    #[test]
    fn localized_name_falls_back_to_the_raw_name() {
        let mut value: serde_json::Value =
            serde_json::from_str(include_str!("subject-example.json")).unwrap();
        value["name"] = serde_json::Value::String("RAW".to_string());
        value["name_cn"] = serde_json::Value::String("中文名".to_string());
        let subject: Subject = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(subject.localized_name(crate::Language::ZhCn), "中文名");
        assert_eq!(subject.localized_name(crate::Language::JaJp), "RAW");
        assert_eq!(subject.localized_name(crate::Language::EnUs), "RAW");

        value["name_cn"] = serde_json::Value::String(String::new());
        let subject: Subject = serde_json::from_value(value).unwrap();
        assert_eq!(subject.localized_name(crate::Language::ZhCn), "RAW");
    }

    #[tokio::test]
    #[ignore = "require-external-internet"]
    async fn request_subject() {
        let ret = Subject::request_with_id(&reqwest::Client::new(), 475_354)
            .await
            .unwrap();
        dbg!(ret);
    }
}
