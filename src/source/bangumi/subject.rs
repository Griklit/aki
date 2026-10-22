use std::path::PathBuf;
use std::{fs::File, io};

use super::BangumiNamespace;
use crate::CONFIG;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// 取条目数据（缓存或接口）时产生的错误。
#[derive(Debug, thiserror::Error)]
pub enum LoadOrRequestError {
    #[error("bangumi request error")]
    Request(#[source] reqwest::Error),
    #[error("save bangumi subject error")]
    Save(#[source] io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub collect: i64,
    pub doing: i64,
    pub dropped: i64,
    pub on_hold: i64,
    pub wish: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Images {
    pub common: String,
    pub grid: String,
    pub large: String,
    pub medium: String,
    pub small: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    String(String),
    Array(Vec<ValueArrayItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueArrayItem {
    pub k: Option<String>,
    pub v: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rating {
    pub count: Count,
    pub rank: i64,
    pub score: f64,
    pub total: i64,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub count: i64,
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
        Self::load_from_path(&BangumiNamespace::subject_path(id))
    }

    /// 从指定文件读条目。
    fn load_from_path(path: &std::path::Path) -> Result<Self, io::Error> {
        let file = File::open(path)?;
        let subject: Subject = serde_json::from_reader(file)?;
        Ok(subject)
    }

    /// 取条目数据：本地已有缓存就用本地的，没有（或要求刷新）才请求接口。
    ///
    /// # Errors
    /// 请求接口失败、或落盘失败时返回错误。
    pub async fn load_or_request_with_id(
        client: &reqwest::Client,
        subject_id: i64,
        refresh: bool,
    ) -> Result<Self, LoadOrRequestError> {
        Self::load_or_request_at(
            client,
            subject_id,
            BangumiNamespace::subject_path(subject_id),
            refresh,
        )
        .await
    }

    /// [`Self::load_or_request_with_id`] 的实现，`path` 由调用方给出。
    async fn load_or_request_at(
        client: &reqwest::Client,
        subject_id: i64,
        path: PathBuf,
        refresh: bool,
    ) -> Result<Self, LoadOrRequestError> {
        if !refresh && path.exists() {
            match Self::load_from_path(&path) {
                Ok(subject) => {
                    eprintln!(
                        "[+] using cached bangumi subject '{}' \
                         (pass --refresh to request it again)",
                        path.display()
                    );
                    return Ok(subject);
                }
                Err(err) => eprintln!(
                    "[?] failed to load cached bangumi subject '{}': {err}, requesting it again",
                    path.display()
                ),
            }
        }
        let subject = Self::request_with_id(client, subject_id)
            .await
            .map_err(LoadOrRequestError::Request)?;
        subject.save().map_err(LoadOrRequestError::Save)?;
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

    /// `code()` 与 `TryFrom<i64>` 互为反向。
    #[test]
    fn type_code_round_trips() {
        for subject_type in [Type::Book, Type::Anime, Type::Music, Type::Game, Type::Real] {
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

    /// 本地已有缓存时不再联网。
    #[tokio::test]
    async fn cached_response_is_reused_without_requesting() {
        let dir = std::env::temp_dir().join(format!("aki-subject-cache-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("475354.json");

        let mut subject: Subject =
            serde_json::from_str(include_str!("subject-example.json")).unwrap();
        subject.name = "CACHED NAME".to_string();
        std::fs::write(&path, serde_json::to_string_pretty(&subject).unwrap()).unwrap();

        let loaded = Subject::load_or_request_at(&reqwest::Client::new(), subject.id, path, false)
            .await
            .unwrap();
        assert_eq!(loaded.name, "CACHED NAME");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    #[ignore = "require-external-internet"]
    async fn request_subject() {
        let ret = Subject::request_with_id(&reqwest::Client::new(), 475_354)
            .await
            .unwrap();
        dbg!(ret);
    }

    /// `refresh` 为 `true` 时忽略缓存、真的重新请求（需要联网）。
    #[tokio::test]
    #[ignore = "require-external-internet"]
    async fn refresh_ignores_the_cached_response() {
        let ret = Subject::load_or_request_with_id(&reqwest::Client::new(), 475_354, true)
            .await
            .unwrap();
        assert_eq!(ret.id, 475_354);
    }
}
