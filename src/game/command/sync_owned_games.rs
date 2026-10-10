//! `sync-owned-games`：把 Steam「已拥有游戏」拉下来，落原始响应，
//! 再据此补全/建立游戏条目。

use super::super::Game;
use super::super::state::{fill_from_steam, minutes_to_seconds};
use super::super::steam;
use crate::Namespace;
use crate::source::steam::{
    OwnedGame, OwnedGamesEndpoint, SteamNamespace, SteamRawStore, SteamRequestError,
    request_with_raw,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

/// 原始响应的存放名：`database/steam/owned-games.json`。
pub const OWNED_GAMES_NAME: &str = "owned-games";

/// 两条 appdetails 请求之间的间隔，避免触发 Steam 的限流。
pub const REQUEST_INTERVAL: Duration = Duration::from_millis(200);

/// 结果里单个条目的处理情况。
#[derive(Debug, Clone)]
pub struct SyncRecord {
    pub appid: u32,
    pub name: Option<String>,
    /// `played_seconds`（分钟 → 秒）。`None` 表示没拿到时长。
    pub played_seconds: Option<u64>,
    pub last_played_at: Option<i64>,
    /// 这次补上的字段名；新建的条目为空。
    pub filled: Vec<&'static str>,
    /// 数据文件里的 `meta.id`（新建或已存在的都有）。
    pub meta_id: Option<String>,
    /// 新建条目的保存失败信息，或 appdetails 失败信息。
    pub error: Option<String>,
    pub created: bool,
    pub existing: bool,
    /// 已拥有、但还没有数据文件（`--no-create`，或建条目的那次请求失败）。
    pub pending: bool,
}

/// `sync-owned-games` 的结果。
#[derive(Debug, Default)]
pub struct SyncResult {
    /// 原始响应落盘路径。
    pub raw_path: Option<PathBuf>,
    /// 库里一共多少个游戏。
    pub owned: usize,
    pub records: Vec<SyncRecord>,
}

impl SyncResult {
    #[must_use]
    pub fn created(&self) -> usize {
        self.records.iter().filter(|r| r.created).count()
    }

    #[must_use]
    pub fn existing(&self) -> usize {
        self.records.iter().filter(|r| r.existing).count()
    }

    #[must_use]
    pub fn filled(&self) -> usize {
        self.records.iter().filter(|r| !r.filled.is_empty()).count()
    }

    /// 真的出错的条目（不含「还没有数据文件」这种正常情况）。
    #[must_use]
    pub fn failed(&self) -> usize {
        self.records.iter().filter(|r| r.error.is_some()).count()
    }

    /// 已拥有、但还没有数据文件的条目。
    #[must_use]
    pub fn pending(&self) -> usize {
        self.records.iter().filter(|r| r.pending).count()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SyncOwnedGamesError {
    #[error("steam request error")]
    Steam(#[source] SteamRequestError),
    #[error("save steam raw data error")]
    RawStore(#[source] std::io::Error),
    #[error("save game error")]
    SaveGame(#[source] super::super::SaveGameError),
}

/// 拉取「已拥有游戏」，**只填 `state` 里为空的字段**，手填的值一律保留。
///
/// `create_missing` 为 `true` 时，给库里还没有条目的游戏补建 `Game`
/// （每条之间隔 [`REQUEST_INTERVAL`]）；为 `false` 时不动数据文件。
///
/// # Errors
/// 拉列表失败、原始响应落盘失败时返回错误；单条 appdetails 失败只记进
/// [`SyncRecord::error`]，不会中断整个同步。
pub async fn sync_owned_games(
    client: &reqwest::Client,
    create_missing: bool,
) -> Result<SyncResult, SyncOwnedGamesError> {
    let (raw, owned) = request_with_raw(client, &OwnedGamesEndpoint::default())
        .await
        .map_err(SyncOwnedGamesError::Steam)?;
    let raw_path = save_raw(raw)?;
    let existing = existing_entries();

    let mut records = Vec::with_capacity(owned.len());
    let owned_count = owned.len();
    for game in owned {
        let record = if let Some((meta_id, path)) = existing.get(&game.appid) {
            sync_existing(&game, meta_id, path)
        } else if create_missing {
            let record = create_game(client, &game).await;
            tokio::time::sleep(REQUEST_INTERVAL).await;
            record
        } else {
            let mut record = record_of(&game);
            record.pending = true;
            record
        };
        records.push(record);
    }

    Ok(SyncResult {
        raw_path: Some(raw_path),
        owned: owned_count,
        records,
    })
}

/// appid → (`meta.id`, 数据文件路径)，只包含已有数据文件的游戏。
fn existing_entries() -> BTreeMap<u32, (String, PathBuf)> {
    let mut existing: BTreeMap<u32, (String, PathBuf)> = BTreeMap::new();
    for (path, game) in Game::iter_all_with_path() {
        let Some(appid) = game.steam_app_id else {
            continue;
        };
        if let Some((previous, _)) = existing.insert(appid, (game.meta.id.id.clone(), path.clone()))
        {
            eprintln!(
                "[?] appid {appid} appears in two files, keeping '{}' and ignoring '{}'",
                previous,
                path.display()
            );
        }
    }
    existing
}

/// 已有条目：只补空字段，真的改了才回写。
fn sync_existing(game: &OwnedGame, meta_id: &str, path: &PathBuf) -> SyncRecord {
    let mut record = record_of(game);
    record.existing = true;
    record.meta_id = Some(meta_id.to_string());
    let mut stored = match Game::load(path) {
        Ok(stored) => stored,
        Err(e) => {
            record.error = Some(e.to_string());
            return record;
        }
    };
    let filled = fill_from_steam(
        &mut stored.state,
        game.playtime_forever,
        game.last_played_at(),
    );
    if filled.is_empty() {
        return record;
    }
    if let Err(e) = stored.save_force(path) {
        record.error = Some(e.to_string());
        return record;
    }
    record.filled = filled;
    record.played_seconds = stored.state.played_seconds();
    record.last_played_at = stored.state.last_played_at();
    record
}

/// 新建条目：先拉 appdetails（拿名称/发售日期等），再填 Steam 的时长。
async fn create_game(client: &reqwest::Client, owned: &OwnedGame) -> SyncRecord {
    let mut record = record_of(owned);
    let app = match steam::request_app_details(client, owned.appid).await {
        Ok(app) => app,
        Err(e) => {
            record.error = Some(e.to_string());
            return record;
        }
    };
    let mut game = match steam::app_details_to_game(&app.raw, owned.appid) {
        Ok(game) => game,
        Err(e) => {
            record.error = Some(e.to_string());
            return record;
        }
    };
    record.filled = fill_from_steam(
        &mut game.state,
        owned.playtime_forever,
        owned.last_played_at(),
    );
    if let Err(e) = game.save_as_meta_id() {
        record.error = Some(SyncOwnedGamesError::SaveGame(e).to_string());
        return record;
    }
    record.created = true;
    record.meta_id = Some(game.meta.id.id.clone());
    record.played_seconds = game.state.played_seconds();
    record.last_played_at = game.state.last_played_at();
    record
}

fn record_of(game: &OwnedGame) -> SyncRecord {
    SyncRecord {
        appid: game.appid,
        name: game.name.clone(),
        played_seconds: Some(minutes_to_seconds(game.playtime_forever)),
        last_played_at: game.last_played_at(),
        filled: Vec::new(),
        meta_id: None,
        error: None,
        created: false,
        existing: false,
        pending: false,
    }
}

fn save_raw(raw: Option<String>) -> Result<PathBuf, SyncOwnedGamesError> {
    let store = SteamRawStore::<Vec<OwnedGame>>::new(owned_games_url(), raw.unwrap_or_default());
    store
        .save_named(OWNED_GAMES_NAME)
        .map_err(SyncOwnedGamesError::RawStore)
}

/// 原始响应文件路径（文件可能还不存在）。
#[must_use]
pub fn owned_games_path() -> PathBuf {
    SteamNamespace::json_path(OWNED_GAMES_NAME)
}

/// `owned-games.json` 的外层形状：`{"response":{"game_count":n,"games":[…]}}`。
#[derive(serde::Deserialize)]
struct OwnedGamesEnvelope {
    response: OwnedGamesResponse,
}

#[derive(serde::Deserialize)]
struct OwnedGamesResponse {
    games: Vec<OwnedGame>,
}

/// 离线读回上次落盘的「已拥有游戏」列表（不联网）。
///
/// 返回 `None` 表示还没同步过。
///
/// # Errors
/// 文件损坏或不是期望的形状时返回错误。
pub fn load_owned_games_raw() -> Result<Option<Vec<OwnedGame>>, SteamRequestError> {
    let raw = match SteamRawStore::<Vec<OwnedGame>>::load_named(OWNED_GAMES_NAME) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(SteamRequestError::RawStore(e)),
    };
    let envelope: OwnedGamesEnvelope =
        serde_json::from_str(&raw).map_err(|source| SteamRequestError::Parse {
            source,
            url: owned_games_url(),
            body: format!("owned games raw ({OWNED_GAMES_NAME})"),
        })?;
    Ok(Some(envelope.response.games))
}

fn owned_games_url() -> String {
    let steam = &crate::CONFIG.steam;
    format!(
        "{}/IPlayerService/GetOwnedGames/v1/?steamid={}",
        steam.api,
        steam.steam_id.as_deref().unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::state::GameState;

    #[test]
    fn minutes_convert_to_seconds() {
        assert_eq!(minutes_to_seconds(25899), 1_553_940);
        assert_eq!(minutes_to_seconds(0), 0);
        assert_eq!(minutes_to_seconds(-1), 0);
    }

    #[test]
    fn unix_seconds_to_date_guards_bad_values() {
        use crate::game::state::unix_seconds_to_date;
        assert_eq!(unix_seconds_to_date(0), None);
        assert_eq!(unix_seconds_to_date(-1), None);
        assert_eq!(unix_seconds_to_date(i64::MAX), None);
        assert!(unix_seconds_to_date(1_553_943_600).is_some());
    }

    /// 同步的核心约定：只填空字段，手填的值绝不被覆盖。
    #[test]
    fn fill_from_steam_never_overwrites() {
        use crate::game::state::fill_from_steam;
        let mut state = GameState::default();
        let filled = fill_from_steam(&mut state, 25899, Some(1_553_943_600));
        assert_eq!(filled, vec!["played_seconds", "last_played_at"]);
        assert_eq!(state.played_seconds(), Some(1_553_940));
        assert_eq!(state.last_played_at(), Some(1_553_943_600));

        // 再同步一次：字段都已填，什么都不做（也不会改动已经写在文件里的值）。
        let filled = fill_from_steam(&mut state, 30000, Some(1_600_000_000));
        assert!(filled.is_empty());
        assert_eq!(state.played_seconds(), Some(1_553_940));
        assert_eq!(state.last_played_at(), Some(1_553_943_600));
    }

    /// 手填的 `play_start_date` / `played_seconds` 与 Steam 的值互不干扰。
    #[test]
    fn hand_filled_fields_survive_sync_on_a_game() {
        use crate::game::state::fill_from_steam;
        let raw = APPDETAILS_FOR_SYNC;
        let mut game = crate::game::steam::app_details_to_game(raw, 570).unwrap();
        let mut hand_state = GameState::default();
        hand_state.playing();
        let hand_start = hand_state.play_start_date();
        game.set_state(hand_state);

        let filled = fill_from_steam(&mut game.state, 25899, Some(1_553_943_600));
        assert_eq!(filled, vec!["played_seconds", "last_played_at"]);
        // 手填的起玩日期没被碰过
        assert_eq!(game.state.play_start_date(), hand_start);
        assert_eq!(
            game.state.play_status(),
            crate::game::GamePlayStatus::Playing
        );
    }

    /// appdetails 的真实响应片段（appid=570）：裸条目。
    const APPDETAILS_FOR_SYNC: &str = r#"{"type":"game","name":"Dota 2","steam_appid":570,"is_free":true,"release_date":{"coming_soon":false,"date":"2013 年 7 月 9 日"}}"#;

    #[test]
    fn zero_playtime_leaves_field_empty() {
        use crate::game::state::fill_from_steam;
        let mut state = GameState::default();
        let filled = fill_from_steam(&mut state, 0, None);
        assert!(filled.is_empty());
        assert_eq!(state.played_seconds(), None);
        assert_eq!(state.last_played_at(), None);
    }

    #[test]
    fn persisted_state_round_trips_through_serde() {
        // `last_played_at` 用 skip_serializing_if，旧文件（没有这个字段）也要能读。
        let old = r#"{"play_status":"plan","played_seconds":null,"achievements":null,
            "comment":null,"play_start_date":null,"play_finish_date":null}"#;
        let state: GameState = serde_json::from_str(old).unwrap();
        assert_eq!(state.last_played_at(), None);

        let new = serde_json::to_string(&state).unwrap();
        assert!(!new.contains("last_played_at"));
    }

    #[test]
    fn offline_loader_reads_saved_response() {
        // 同步过一次就应当能离线读回；没同步过则返回 None。
        match load_owned_games_raw() {
            Ok(Some(games)) => assert!(!games.is_empty()),
            Ok(None) => eprintln!("[?] skip: no owned-games.json yet"),
            Err(e) => panic!("offline loader failed: {e}"),
        }
    }

    #[test]
    fn owned_game_last_played_filters_sentinels() {
        let mut game = OwnedGame {
            appid: 570,
            name: None,
            playtime_forever: 0,
            playtime_2weeks: None,
            rtime_last_played: Some(0),
            playtime_disconnected: None,
            img_icon_url: None,
        };
        assert_eq!(game.last_played_at(), None);
        game.rtime_last_played = Some(-1);
        assert_eq!(game.last_played_at(), None);
        game.rtime_last_played = Some(1_553_943_600);
        assert_eq!(game.last_played_at(), Some(1_553_943_600));
    }
}
