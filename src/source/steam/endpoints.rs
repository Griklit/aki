use super::SteamRawStore;
use super::http::{Endpoint, SteamRequestError, parse_json};
use super::model::{AppDetails, OwnedGame, PlayerSummary};
use crate::CONFIG;
use serde::Deserialize;
use std::collections::BTreeMap;

/// 老式外壳 `{"<appid>": {"success": bool, "data": {...}}}`。
#[derive(Debug, Deserialize)]
struct AppDetailsEnvelope {
    success: bool,
    data: Option<serde_json::Value>,
}

/// 只取 `response` 字段的接口（Steam Web API 几乎都是这个形状）。
macro_rules! response_map {
    ($outer:ident => $inner:ty) => {
        #[derive(Debug, Deserialize)]
        struct $outer {
            response: $inner,
        }
    };
}

response_map!(OwnedGamesEnvelope => OwnedGamesResponse);
response_map!(PlayerSummariesEnvelope => PlayerSummariesResponse);
response_map!(CurrentPlayersEnvelope => CurrentPlayersResponse);

#[derive(Debug, Deserialize)]
struct OwnedGamesResponse {
    games: Vec<OwnedGame>,
}

#[derive(Debug, Deserialize)]
struct PlayerSummariesResponse {
    players: Vec<PlayerSummary>,
}

#[derive(Debug, Deserialize)]
struct CurrentPlayersResponse {
    player_count: u32,
    result: i32,
}

/// 商店接口：`/api/appdetails`。
#[derive(Debug, Clone)]
pub struct AppDetailsEndpoint {
    pub appid: u32,
}

impl AppDetailsEndpoint {
    #[must_use]
    pub const fn new(appid: u32) -> Self {
        Self { appid }
    }
}

impl Endpoint for AppDetailsEndpoint {
    type Response = AppDetails;

    fn path(&self) -> String {
        "/api/appdetails".to_string()
    }

    fn params(&self) -> Vec<(&'static str, String)> {
        let steam_config = &CONFIG.steam;
        vec![
            ("appids", self.appid.to_string()),
            ("l", steam_config.language.clone()),
            ("cc", steam_config.country.clone()),
        ]
    }

    fn use_web_api(&self) -> bool {
        false
    }

    fn parse(&self, body: &str) -> Result<Self::Response, SteamRequestError> {
        Ok(extract_checked(body, self.appid)?.0)
    }

    /// 落盘的是响应原文，一个字节都不改。
    fn raw_body(&self, body: &str) -> Result<Option<String>, SteamRequestError> {
        let (_, raw) = extract_checked(body, self.appid)?;
        Ok(Some(raw))
    }
}

/// 取出 appdetails 的条目数据，并校验 appid。
///
/// 返回 `(解析后的条目, 要落盘的文本)`。
fn extract_checked(body: &str, appid: u32) -> Result<(AppDetails, String), SteamRequestError> {
    let data = extract_app_details(body, appid)?;
    let app: AppDetails =
        serde_json::from_value(data).map_err(|source| SteamRequestError::Parse {
            source,
            url: String::new(),
            body: format!("appdetails data of appid {appid}"),
        })?;
    if app.steam_appid != appid {
        return Err(SteamRequestError::ResponseAppIdMismatch {
            expected: appid,
            actual: app.steam_appid,
        });
    }
    Ok((app, body.to_string()))
}

/// 从响应里取出条目数据（[`serde_json::Value`]）。
///
/// 兼容裸对象与老式外壳两种形状。
///
/// # Errors
/// JSON 非法，或请求的 appid 不存在时返回错误。
fn extract_app_details(body: &str, appid: u32) -> Result<serde_json::Value, SteamRequestError> {
    // 先判断是不是老式外壳：外壳的标志是顶层有数字 appid 作为键。
    let is_envelope =
        serde_json::from_str::<BTreeMap<String, serde_json::Value>>(body).is_ok_and(|map| {
            map.keys()
                .any(|key| key.chars().all(|c| c.is_ascii_digit()))
        });
    if !is_envelope {
        return serde_json::from_str(body).map_err(|source| SteamRequestError::Parse {
            source,
            url: String::new(),
            body: format!("appdetails response of appid {appid}"),
        });
    }
    let mut shell: BTreeMap<String, AppDetailsEnvelope> = parse_json(body)?;
    let Some(envelope) = shell.remove(&appid.to_string()) else {
        return Err(SteamRequestError::AppNotFound { appid });
    };
    if !envelope.success {
        return Err(SteamRequestError::AppNotFound { appid });
    }
    envelope
        .data
        .ok_or(SteamRequestError::AppNotFound { appid })
}
/// Web API：`IPlayerService/GetOwnedGames`（需要密钥与 `steam_id`）。
#[derive(Debug, Clone, Copy)]
pub struct OwnedGamesEndpoint {
    pub include_app_info: bool,
    pub include_played_free_games: bool,
}

impl Default for OwnedGamesEndpoint {
    fn default() -> Self {
        Self {
            include_app_info: true,
            include_played_free_games: true,
        }
    }
}

impl Endpoint for OwnedGamesEndpoint {
    type Response = Vec<OwnedGame>;

    fn path(&self) -> String {
        "/IPlayerService/GetOwnedGames/v1/".to_string()
    }

    fn params(&self) -> Vec<(&'static str, String)> {
        let Some(steam_id) = CONFIG.steam.steam_id.as_deref() else {
            return vec![("steamid", String::new())];
        };
        vec![
            ("steamid", steam_id.to_string()),
            (
                "include_appinfo",
                u8::from(self.include_app_info).to_string(),
            ),
            (
                "include_played_free_games",
                u8::from(self.include_played_free_games).to_string(),
            ),
        ]
    }

    /// 没配置 `SteamID` 就直接报错，而不是带空 `steamid` 去请求。
    fn validate(&self) -> Result<(), SteamRequestError> {
        if CONFIG
            .steam
            .steam_id
            .as_deref()
            .unwrap_or_default()
            .is_empty()
        {
            return Err(SteamRequestError::MissingConfig {
                name: "steam.steam_id",
            });
        }
        Ok(())
    }

    fn use_web_api(&self) -> bool {
        true
    }

    fn needs_api_key(&self) -> bool {
        true
    }

    fn parse(&self, body: &str) -> Result<Self::Response, SteamRequestError> {
        let envelope: OwnedGamesEnvelope = parse_json(body)?;
        Ok(envelope.response.games)
    }

    /// 落盘响应原文。
    fn raw_body(&self, body: &str) -> Result<Option<String>, SteamRequestError> {
        self.parse(body)?;
        Ok(Some(body.to_string()))
    }
}

/// Web API：`ISteamUser/GetPlayerSummaries`（需要密钥）。
#[derive(Debug, Clone)]
pub struct PlayerSummariesEndpoint {
    pub steam_ids: Vec<String>,
}

impl PlayerSummariesEndpoint {
    #[must_use]
    pub const fn new(steam_ids: Vec<String>) -> Self {
        Self { steam_ids }
    }
}

impl Endpoint for PlayerSummariesEndpoint {
    type Response = Vec<PlayerSummary>;

    fn path(&self) -> String {
        "/ISteamUser/GetPlayerSummaries/v2/".to_string()
    }

    fn params(&self) -> Vec<(&'static str, String)> {
        vec![("steamids", self.steam_ids.join(","))]
    }

    fn use_web_api(&self) -> bool {
        true
    }

    fn needs_api_key(&self) -> bool {
        true
    }

    fn parse(&self, body: &str) -> Result<Self::Response, SteamRequestError> {
        let envelope: PlayerSummariesEnvelope = parse_json(body)?;
        Ok(envelope.response.players)
    }
}

/// Web API：`ISteamUserStats/GetNumberOfCurrentPlayers`（不需要密钥）。
#[derive(Debug, Clone, Copy)]
pub struct CurrentPlayersEndpoint {
    pub appid: u32,
}

impl CurrentPlayersEndpoint {
    #[must_use]
    pub const fn new(appid: u32) -> Self {
        Self { appid }
    }
}

impl Endpoint for CurrentPlayersEndpoint {
    type Response = u32;

    fn path(&self) -> String {
        "/ISteamUserStats/GetNumberOfCurrentPlayers/v1/".to_string()
    }

    fn params(&self) -> Vec<(&'static str, String)> {
        vec![("appid", self.appid.to_string())]
    }

    fn use_web_api(&self) -> bool {
        true
    }

    fn parse(&self, body: &str) -> Result<Self::Response, SteamRequestError> {
        let envelope: CurrentPlayersEnvelope = parse_json(body)?;
        if envelope.response.result != 1 {
            return Err(SteamRequestError::AppNotFound { appid: self.appid });
        }
        Ok(envelope.response.player_count)
    }
}

/// appdetails 原始响应的存取类型别名。
pub type AppDetailsRaw = SteamRawStore<AppDetails>;

#[cfg(test)]
mod tests {
    use super::*;

    /// appdetails 的真实响应片段（appid=570）：**裸条目**，没有
    /// `{"570":{"success":true,"data":…}}` 那层外壳。
    const APPDETAILS: &str = r#"{"type":"game","name":"Dota 2","steam_appid":570,"required_age":0,"is_free":true,"short_description":"每一天全球有数百万玩家化为一百余名Dota英雄展开大战。","developers":["Valve"],"publishers":["Valve"],"platforms":{"windows":true,"mac":true,"linux":true},"metacritic":{"score":90,"url":"https://www.metacritic.com/game/pc/dota-2"},"dlc":[1241930,652720],"categories":[{"id":1,"description":"多人"}],"genres":[{"id":"1","description":"动作"},{"id":"2","description":"策略"}],"screenshots":[{"id":0,"path_thumbnail":"https://example.com/t.jpg","path_full":"https://example.com/f.jpg"}],"recommendations":{"total":14374},"release_date":{"coming_soon":false,"date":"2013 年 7 月 9 日"},"header_image":"https://example.com/header.jpg"}"#;

    /// 同一份条目的**老式外壳版**（字段更多，Steam 有时这么回）。
    const APPDETAILS_ENVELOPE: &str = r#"{"570":{"success":true,"data":{"type":"game","name":"Dota 2","steam_appid":570,"required_age":0,"is_free":true,"short_description":"每一天全球有数百万玩家化为一百余名Dota英雄展开大战。","price_overview":{"currency":"CNY","final_formatted":"免费"},"developers":["Valve"],"publishers":["Valve"],"platforms":{"windows":true,"mac":true,"linux":true},"metacritic":{"score":90,"url":"https://www.metacritic.com/game/pc/dota-2"},"dlc":[1241930,652720],"categories":[{"id":1,"description":"多人"}],"genres":[{"id":"1","description":"动作"},{"id":"2","description":"策略"}],"recommendations":{"total":14374},"release_date":{"coming_soon":false,"date":"2013 年 7 月 9 日"},"header_image":"https://example.com/header.jpg"}}}"#;

    #[test]
    fn parse_app_details() {
        let endpoint = AppDetailsEndpoint::new(570);
        let app = endpoint.parse(APPDETAILS).unwrap();
        assert_eq!(app.steam_appid, 570);
        assert_eq!(app.name, "Dota 2");
        assert!(app.is_free);
        assert_eq!(app.recommendations.as_ref().unwrap().total, 14374);
        assert_eq!(
            app.release_date.as_ref().unwrap().date,
            chrono::NaiveDate::from_ymd_opt(2013, 7, 9)
        );
    }

    #[test]
    fn parse_enveloped_app_details() {
        let endpoint = AppDetailsEndpoint::new(570);
        let app = endpoint.parse(APPDETAILS_ENVELOPE).unwrap();
        assert_eq!(app.steam_appid, 570);
        assert_eq!(app.name, "Dota 2");
        assert_eq!(app.recommendations.as_ref().unwrap().total, 14374);
    }

    #[test]
    fn raw_body_keeps_unmodelled_fields() {
        let endpoint = AppDetailsEndpoint::new(570);
        // 落盘的是响应原文：字节完全一致，没进结构体的字段（dlc 等）也在。
        let raw = endpoint.raw_body(APPDETAILS).unwrap().unwrap();
        assert_eq!(raw, APPDETAILS);

        let raw = endpoint.raw_body(APPDETAILS_ENVELOPE).unwrap().unwrap();
        assert_eq!(raw, APPDETAILS_ENVELOPE);
        assert!(raw.contains("\"price_overview\""));

        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["570"]["data"]["name"], "Dota 2");
    }

    #[test]
    fn app_id_mismatch_is_error() {
        let endpoint = AppDetailsEndpoint::new(570);
        let err = endpoint
            .parse(r#"{"steam_appid":999,"name":"Other","type":"game"}"#)
            .expect_err("wrong appid should not parse");
        assert!(matches!(
            err,
            SteamRequestError::ResponseAppIdMismatch {
                expected: 570,
                actual: 999
            }
        ));
    }

    #[test]
    fn required_age_accepts_number_and_string() {
        let endpoint = AppDetailsEndpoint::new(570);
        let number = endpoint
            .parse(r#"{"steam_appid":570,"name":"A","type":"game","required_age":17}"#)
            .unwrap();
        assert_eq!(number.required_age, 17);
        let text = endpoint
            .parse(r#"{"steam_appid":570,"name":"A","type":"game","required_age":"18"}"#)
            .unwrap();
        assert_eq!(text.required_age, 18);
        let absent = endpoint
            .parse(r#"{"steam_appid":570,"name":"A","type":"game"}"#)
            .unwrap();
        assert_eq!(absent.required_age, 0);
    }

    #[test]
    fn missing_app_is_not_found() {
        let endpoint = AppDetailsEndpoint::new(570);
        let err = endpoint
            .parse(r#"{"570":{"success":false}}"#)
            .expect_err("success=false should not parse");
        assert!(matches!(err, SteamRequestError::AppNotFound { appid: 570 }));
    }

    #[test]
    fn malformed_body_is_parse_error() {
        let endpoint = AppDetailsEndpoint::new(570);
        let err = endpoint
            .parse(r#"{"name":"no appid"}"#)
            .expect_err("parse error");
        assert!(matches!(err, SteamRequestError::Parse { .. }));
    }

    #[test]
    fn parse_owned_games() {
        let endpoint = OwnedGamesEndpoint::default();
        let games = endpoint
            .parse(r#"{"response":{"game_count":2,"games":[{"appid":570,"name":"Dota 2","playtime_forever":1347},{"appid":620,"playtime_forever":0}]}}"#)
            .unwrap();
        assert_eq!(games.len(), 2);
        assert_eq!(games[1].appid, 620);
        assert_eq!(games[1].name, None);
    }
}
