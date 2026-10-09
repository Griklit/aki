use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Steam 商店 `appdetails` 返回的条目类型。
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppType {
    Game,
    Dlc,
    Demo,
    Music,
    Tool,
    Application,
    Video,
    Series,
    Hardware,
    #[serde(other)]
    Other,
}

/// `release_date`。Steam 的 `date` 是脏字符串，这里同时保留原文和「能解析到哪一步」。
///
/// - `date`：精确到日（如 `"9 Jul, 2013"`、`"2013 年 7 月 9 日"`）时才有值
/// - `year` / `month` / `quarter`：只有粒度时填哪一级
/// - `date_text`：Steam 原文，解析不出来时用它兜底，不丢信息
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ReleaseDate {
    #[serde(default)]
    pub coming_soon: bool,
    #[serde(default)]
    pub date_text: Option<String>,
    #[serde(default, deserialize_with = "deserialize_steam_date")]
    pub date: Option<NaiveDate>,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub month: Option<u8>,
    #[serde(default)]
    pub quarter: Option<u8>,
}

impl ReleaseDate {
    #[must_use]
    pub fn new(raw: Option<String>, coming_soon: bool) -> Self {
        let text = raw.unwrap_or_default();
        Self {
            coming_soon,
            date: super::date::parse_date(&text),
            year: super::date::detect_year(&text),
            month: super::date::detect_month(&text),
            quarter: super::date::detect_quarter(&text),
            date_text: if text.is_empty() { None } else { Some(text) },
        }
    }
}

fn deserialize_steam_date<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.as_deref().and_then(super::date::parse_steam_date))
}

/// 反序列化 `release_date`：把脏的 `date` 字符串同时喂给
/// [`ReleaseDate::new`]，这样年月与原文都能留下来。
fn deserialize_release_date<'de, D>(deserializer: D) -> Result<Option<ReleaseDate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        date: Option<String>,
        #[serde(default)]
        coming_soon: bool,
    }
    let raw = Option::<Raw>::deserialize(deserializer)?;
    Ok(raw.map(|r| ReleaseDate::new(r.date, r.coming_soon)))
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Platforms {
    #[serde(default)]
    pub windows: bool,
    #[serde(default)]
    pub mac: bool,
    #[serde(default)]
    pub linux: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Genre {
    #[serde(default)]
    pub id: String,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Category {
    #[serde(default)]
    pub id: u32,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SteamRecommendations {
    #[serde(default)]
    pub total: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Metacritic {
    pub score: u32,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Screenshot {
    #[serde(default)]
    pub id: u32,
    pub path_thumbnail: String,
    pub path_full: String,
}

/// `store.steampowered.com/api/appdetails` 返回的条目。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppDetails {
    pub steam_appid: u32,
    pub name: String,
    pub r#type: AppType,
    #[serde(default)]
    pub is_free: bool,
    #[serde(default)]
    pub detailed_description: String,
    #[serde(default)]
    pub short_description: String,
    #[serde(default)]
    pub developers: Vec<String>,
    #[serde(default)]
    pub publishers: Vec<String>,
    #[serde(default)]
    pub platforms: Platforms,
    #[serde(default)]
    pub genres: Vec<Genre>,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default, deserialize_with = "deserialize_release_date")]
    pub release_date: Option<ReleaseDate>,
    #[serde(default)]
    pub header_image: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<Screenshot>,
    #[serde(default)]
    pub recommendations: Option<SteamRecommendations>,
    #[serde(default)]
    pub metacritic: Option<Metacritic>,
    /// Steam 有时给数字 `0`，有时给字符串 `"17"`（分级游戏），两种都要认。
    #[serde(default, deserialize_with = "deserialize_age")]
    pub required_age: u32,
}

/// `required_age` 既可能是 `17` 也可能是 `"17"`。
fn deserialize_age<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Age {
        Number(u64),
        Text(String),
    }
    let Some(age) = Option::<Age>::deserialize(deserializer)? else {
        return Ok(0);
    };
    Ok(match age {
        Age::Number(value) => u32::try_from(value).unwrap_or_default(),
        Age::Text(text) => text.trim().parse().unwrap_or_default(),
    })
}

/// `IPlayerService/GetOwnedGames` 里的单个游戏。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OwnedGame {
    pub appid: u32,
    #[serde(default)]
    pub name: Option<String>,
    /// 总游玩时长，单位：分钟（Steam 原始单位）。用 `i64` 是因为 Steam 会给出
    /// `-1` 之类的哨兵值，用 `u32` 会让整个响应反序列化失败。
    #[serde(default)]
    pub playtime_forever: i64,
    /// 最近两周游玩时长，单位：分钟。
    #[serde(default)]
    pub playtime_2weeks: Option<i64>,
    /// 最后一次游玩时间（Unix 秒），`0` 表示从未玩过。
    #[serde(default)]
    pub rtime_last_played: Option<i64>,
    /// 因为离线而没计入的时长（分钟），Steam 单独给。
    #[serde(default)]
    pub playtime_disconnected: Option<i64>,
    #[serde(default)]
    pub img_icon_url: Option<String>,
}

impl OwnedGame {
    /// 最后一次游玩时间，过滤掉 Steam 的哨兵值（`0` / 负数）。
    #[must_use]
    pub fn last_played_at(&self) -> Option<i64> {
        self.rtime_last_played.filter(|seconds| *seconds > 0)
    }
}

/// `ISteamUser/GetPlayerSummaries` 里的单个玩家。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayerSummary {
    pub steamid: String,
    pub personaname: String,
    #[serde(default)]
    pub profileurl: Option<String>,
    #[serde(default)]
    pub avatarfull: Option<String>,
    #[serde(default)]
    pub communityvisibilitystate: Option<u32>,
}

/// `SteamID64` → `SteamID3`（`[U:1:xxx]` 里的 `xxx`）。
#[must_use]
pub fn steamid64_to_account_id(steamid64: &str) -> Option<u32> {
    const BASE: u64 = 76_561_197_960_265_728;
    let id: u64 = steamid64.parse().ok()?;
    u32::try_from(id.checked_sub(BASE)?).ok()
}
