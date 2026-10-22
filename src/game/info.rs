use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// 发售日期。
///
/// 游戏发售日期粒度不一（年份/年月/季度等），不能用 `NaiveDate` 表示。
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GameRelease {
    /// 精确到日。
    Exact { date: NaiveDate },
    /// 只知道年份。
    Year { year: i32 },
    /// 知道年月，不知道日。
    YearMonth { year: i32, month: u8 },
    /// 只知道季度。
    Quarter { year: i32, quarter: u8 },
    /// 无法解析时原样保留，不丢信息。
    Text { raw: String },
    /// 尚未发售（Steam 的 `coming_soon`）。
    Tba,
}

impl GameRelease {
    /// 用于 `meta.id` 的日期前缀，粒度不足时给出对应粒度的字符串。
    #[must_use]
    pub fn date_prefix(&self) -> String {
        match self {
            Self::Exact { date } => date.format("%Y-%m-%d").to_string(),
            Self::Year { year } => year.to_string(),
            Self::YearMonth { year, month } => format!("{year}-{month:02}"),
            Self::Quarter { year, quarter } => format!("{year}-Q{quarter}"),
            Self::Text { raw } => raw.clone(),
            Self::Tba => "unknown".to_string(),
        }
    }

    /// 粒度是否精确到日。
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        matches!(self, Self::Exact { .. })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum GamePlatform {
    Windows,
    Mac,
    Linux,
}

impl GamePlatform {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Mac => "mac",
            Self::Linux => "linux",
        }
    }
}

impl TryFrom<&str> for GamePlatform {
    type Error = crate::utils::error::EnumVariantNotFound;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "windows" | "win" | "pc" => Ok(Self::Windows),
            "mac" | "macos" | "osx" | "os x" => Ok(Self::Mac),
            "linux" => Ok(Self::Linux),
            _ => Err(crate::utils::error::EnumVariantNotFound),
        }
    }
}

/// 游戏基本信息，与 [`AnimeInfo`](crate::AnimeInfo) 对应。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GameInfo {
    pub title: String,
    pub summary: Option<String>,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    pub release: Option<GameRelease>,
    pub platforms: Vec<GamePlatform>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub is_free: bool,
    pub website: Option<String>,
    /// 封面图 URL（Steam 的 `header_image`）。
    pub cover: Option<String>,
    pub screenshots: Vec<String>,
    /// 外部评分（Metacritic 等）。
    pub external_score: Option<ExternalScore>,
    /// 评论总数。
    pub review_count: Option<u32>,
}

impl GameInfo {
    #[must_use]
    pub fn publisher(&self) -> Option<&str> {
        self.publishers.first().map(String::as_str)
    }

    #[must_use]
    pub fn developer(&self) -> Option<&str> {
        self.developers.first().map(String::as_str)
    }

    #[must_use]
    pub fn release_prefix(&self) -> String {
        self.release
            .as_ref()
            .map_or_else(|| "unknown".to_string(), GameRelease::date_prefix)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GameInfoInternationalization {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl GameInfoInternationalization {
    #[must_use]
    pub const fn new(title: Option<String>, summary: Option<String>) -> Self {
        Self { title, summary }
    }
}

/// 第三方给的分，只做展示，不参与项目自身的评分计算。
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ExternalScore {
    pub source: String,
    pub score: f32,
    /// 原始满分，如 Metacritic 的 100。
    pub max: f32,
    pub url: Option<String>,
}
