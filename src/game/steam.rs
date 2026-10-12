//! Steam → 游戏领域模型的转换，以及原始响应的存取入口。
//!
//! 接口调用与响应模型在 [`crate::steam`]；这里只做「Steam 的数据怎么变成
//! [`Game`]」这一件事。

use crate::game::Game;
use crate::game::info::{
    ExternalScore, GameInfo, GameInfoInternationalization, GamePlatform, GameRelease,
};
use crate::game::state::GameState;
use crate::source::steam::{
    AppDetails, AppDetailsEndpoint, Endpoint, SteamNamespace, SteamRawStore, SteamRequestError,
    model, request_with_raw,
};
use crate::{Language, Meta};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 一条 appdetails 原始响应 + 它的请求地址。
pub type AppDetailsRawStore = SteamRawStore<AppDetails>;

/// 原始响应的存放名：`database/steam/appdetails/{appid}.json`。
#[must_use]
pub fn app_details_name(appid: u32) -> String {
    format!("appdetails/{appid}")
}

/// 该 appid 的原始响应文件路径（文件可能还不存在）。
#[must_use]
pub fn app_details_path(appid: u32) -> PathBuf {
    SteamNamespace::saved_path(&app_details_name(appid))
}

/// 把 Steam 的脏日期字段转成 [`GameRelease`]。
///
/// 精度按 日 → 年月 → 年+季度 → 年 → 原文 依次降级，任何一步都不丢信息。
#[must_use]
pub fn parse_release(release_date: &model::ReleaseDate) -> GameRelease {
    if release_date.coming_soon {
        return GameRelease::Tba;
    }
    if let Some(date) = release_date.date {
        return GameRelease::Exact { date };
    }
    match (release_date.year, release_date.month, release_date.quarter) {
        (Some(year), Some(month), _) => GameRelease::YearMonth { year, month },
        (Some(year), None, Some(quarter)) => GameRelease::Quarter { year, quarter },
        (Some(year), None, None) => GameRelease::Year { year },
        _ => GameRelease::Text {
            raw: release_date
                .date_text
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        },
    }
}

/// 解析 Steam 的 appdetails 响应，得到 [`AppDetails`]。
///
/// 两种形状都认（裸条目与 `{"<appid>":{"success":true,"data":{...}}}` 外壳），
/// 具体判断在 [`Endpoint::parse`] 里，所以联网与离线走的是同一套逻辑。
///
/// # Errors
/// JSON 非法、缺少必要字段、或响应里的 appid 与请求不一致时返回错误。
pub fn parse_app_details(body: &str, appid: u32) -> Result<AppDetails, SteamRequestError> {
    AppDetailsEndpoint::new(appid).parse(body)
}

/// 解析落盘的原始响应文件（`database/steam/appdetails427520.json` 之类）。
///
/// 文件里存的是响应原文，与 [`parse_app_details`] 完全同一份内容，
/// 保留这个名字只是为了调用处读起来更清楚。
///
/// # Errors
/// 同 [`parse_app_details`]。
pub fn parse_app_details_data(raw: &str, appid: u32) -> Result<AppDetails, SteamRequestError> {
    parse_app_details(raw, appid)
}

/// 解析原始响应并直接转成 [`Game`]。
///
/// # Errors
/// 同 [`parse_app_details`]。
pub fn app_details_to_game(body: &str, appid: u32) -> Result<Game, SteamRequestError> {
    Ok(Game::from(parse_app_details(body, appid)?))
}

/// 请求一条 appdetails，并把**原始响应**存到
/// `database/steam/appdetails/{appid}.json`（和 Bangumi 存 `Subject` 原样响应同理）。
///
/// 存的是响应里的 `data` 原文，不是解析后再序列化的结果，所以没进结构体的字段
/// （价格、配置需求、DLC 列表等）也在文件里，以后补字段可以离线重跑。
///
/// # Errors
/// 网络失败、Steam 返回非 2xx、响应不是期望的形状、或落盘失败时返回错误。
pub async fn request_app_details(
    client: &reqwest::Client,
    appid: u32,
) -> Result<AppDetailsRawStore, SteamRequestError> {
    let (raw, _app) = request_with_raw(client, &AppDetailsEndpoint::new(appid)).await?;
    let steam_config = &crate::CONFIG.steam;
    let url = format!(
        "{}/api/appdetails?appids={}&l={}&cc={}",
        steam_config.store_api, appid, steam_config.language, steam_config.country
    );
    let store = AppDetailsRawStore::new(url, raw.unwrap_or_default());
    store
        .save_named(&app_details_name(appid))
        .map_err(SteamRequestError::RawStore)?;
    Ok(store)
}

/// 从 `database/steam/appdetails/{appid}.json` 离线读回并解析。
///
/// 文件里存的是 `data` 原文，所以走 [`parse_app_details_data`] 而不是
/// [`parse_app_details`]。
///
/// # Errors
/// 文件不存在、读取失败、JSON 非法或缺少必要字段时返回错误。
pub fn load_app_details(appid: u32) -> Result<AppDetails, SteamRequestError> {
    let raw = AppDetailsRawStore::load_named(&app_details_name(appid))
        .map_err(SteamRequestError::RawStore)?;
    parse_app_details_data(&raw, appid)
}

impl From<AppDetails> for Game {
    fn from(app: AppDetails) -> Self {
        let title = app.name.clone();
        let release = app.release_date.as_ref().map(parse_release);
        let summary = Some(app.short_description.clone());
        let detected_language = Language::detect_from(&title);
        let mut internationalizations = BTreeMap::new();
        internationalizations.insert(
            detected_language,
            GameInfoInternationalization::new(Some(title.clone()), summary.clone()),
        );
        let meta_id = format!(
            "[{}] {}",
            release
                .as_ref()
                .map_or_else(|| "unknown".to_string(), GameRelease::date_prefix),
            title
        );
        let cover = app.header_image.clone();
        let screenshots = app
            .screenshots
            .iter()
            .map(|s| s.path_full.clone())
            .collect();
        let external_score = app.metacritic.as_ref().map(|m| ExternalScore {
            source: "metacritic".to_string(),
            // Metacritic 是 0~100 的整数分，转 f32 只为算比例，精度损失无所谓。
            #[allow(clippy::cast_precision_loss)]
            score: m.score as f32,
            max: 100.0,
            url: m.url.clone(),
        });
        let review_count = app.recommendations.as_ref().map(|r| r.total);
        Self {
            meta: Meta::with_id(meta_id, Some(title.clone())),
            info: GameInfo {
                title,
                summary,
                developers: app.developers.clone(),
                publishers: app.publishers.clone(),
                release,
                platforms: platforms_of(&app.platforms),
                genres: app.genres.iter().map(|g| g.description.clone()).collect(),
                tags: app
                    .categories
                    .iter()
                    .map(|c| c.description.clone())
                    .collect(),
                is_free: app.is_free,
                website: app.website.clone(),
                cover,
                screenshots,
                external_score,
                review_count,
            },
            internationalizations,
            state: GameState::default(),
            steam_app_id: Some(app.steam_appid),
            bangumi_id: None,
        }
    }
}

fn platforms_of(platforms: &model::Platforms) -> Vec<GamePlatform> {
    let mut result = Vec::new();
    if platforms.windows {
        result.push(GamePlatform::Windows);
    }
    if platforms.mac {
        result.push(GamePlatform::Mac);
    }
    if platforms.linux {
        result.push(GamePlatform::Linux);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::state::GamePlayStatus;

    /// appdetails 的真实响应片段（appid=570）：裸条目，无 `{appid:{success,data}}` 外壳。
    const APPDETAILS: &str = r#"{"type":"game","name":"Dota 2","steam_appid":570,"required_age":0,"is_free":true,"short_description":"每一天全球有数百万玩家化为一百余名Dota英雄展开大战。","developers":["Valve"],"publishers":["Valve"],"platforms":{"windows":true,"mac":true,"linux":true},"metacritic":{"score":90,"url":"https://www.metacritic.com/game/pc/dota-2"},"dlc":[1241930,652720],"categories":[{"id":1,"description":"多人"}],"genres":[{"id":"1","description":"动作"},{"id":"2","description":"策略"}],"screenshots":[{"id":0,"path_thumbnail":"https://example.com/t.jpg","path_full":"https://example.com/f.jpg"}],"recommendations":{"total":14374},"release_date":{"coming_soon":false,"date":"2013 年 7 月 9 日"},"header_image":"https://example.com/header.jpg"}"#;

    const APPDETAILS_TBA: &str = r#"{"type":"game","name":"Some Game","steam_appid":570,"release_date":{"coming_soon":true,"date":"即将推出"}}"#;

    const APPDETAILS_YEAR: &str = r#"{"type":"game","name":"Some Game","steam_appid":570,"release_date":{"coming_soon":false,"date":"2024"}}"#;

    const APPDETAILS_QUARTER: &str = r#"{"type":"game","name":"Some Game","steam_appid":570,"release_date":{"coming_soon":false,"date":"2025 年 第 1 季度"}}"#;

    #[test]
    fn convert_app_details_to_game() {
        let game = app_details_to_game(APPDETAILS, 570).unwrap();
        assert_eq!(game.info.title, "Dota 2");
        assert_eq!(game.info.release_prefix(), "2013-07-09");
        assert_eq!(game.meta.id.id, "[2013-07-09] Dota 2");
        assert_eq!(game.meta.data.title.as_deref(), Some("Dota 2"));
        assert_eq!(game.info.developer(), Some("Valve"));
        assert_eq!(game.info.platforms.len(), 3);
        assert_eq!(game.info.genres, vec!["动作", "策略"]);
        assert_eq!(game.info.review_count, Some(14_374));
        let score = game.info.external_score.unwrap();
        assert_eq!(score.source, "metacritic");
        assert!((score.score - 90.0).abs() < f32::EPSILON);
        assert_eq!(game.steam_app_id, Some(570));
        assert_eq!(game.state.play_status(), GamePlayStatus::Plan);
        assert_eq!(game.state.played_seconds(), None);
        assert_eq!(game.state.achievements(), None);
    }

    #[test]
    fn release_precision_falls_back() {
        let tba = app_details_to_game(APPDETAILS_TBA, 570).unwrap();
        assert_eq!(tba.info.release, Some(GameRelease::Tba));
        assert_eq!(tba.meta.id.id, "[unknown] Some Game");

        let year = app_details_to_game(APPDETAILS_YEAR, 570).unwrap();
        assert_eq!(year.info.release, Some(GameRelease::Year { year: 2024 }));

        let quarter = app_details_to_game(APPDETAILS_QUARTER, 570).unwrap();
        assert_eq!(
            quarter.info.release,
            Some(GameRelease::Quarter {
                year: 2025,
                quarter: 1
            })
        );
        assert_eq!(quarter.info.release_prefix(), "2025-Q1");
    }

    #[test]
    fn wrong_appid_is_error() {
        assert!(
            app_details_to_game(r#"{"steam_appid":999,"name":"Other","type":"game"}"#, 570)
                .is_err()
        );
        assert!(app_details_to_game("not json", 570).is_err());
    }

    /// 用真实落盘文件复现：appdetails 外壳版（顶层是数字 appid）。
    #[test]
    fn parse_real_saved_factorio() {
        let path = app_details_path(427_520);
        let Ok(raw) = std::fs::read_to_string(&path) else {
            eprintln!("[?] skip: {} not found", path.display());
            return;
        };
        let game = app_details_to_game(&raw, 427_520).expect("real response should parse");
        assert_eq!(game.info.title, "Factorio");
        assert_eq!(game.steam_app_id, Some(427_520));
        eprintln!("[#] parsed real response: {}", game.meta.id.id);
    }
}
