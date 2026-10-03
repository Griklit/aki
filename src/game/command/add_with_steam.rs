use super::super::{Game, SaveGameError, steam};
use crate::steam::SteamRequestError;

#[derive(Debug, thiserror::Error)]
pub enum AddWithSteamError {
    #[error("steam request error")]
    Steam(#[from] SteamRequestError),
    #[error("save game error")]
    SaveGame(#[source] SaveGameError),
}

/// 按 Steam appid 添加一个游戏：请求商店接口、存原始响应、再存游戏数据文件。
///
/// # Errors
/// 网络失败、appid 不存在、或落盘失败时返回错误。
pub async fn add_with_steam(
    client: &reqwest::Client,
    appid: u32,
) -> Result<Game, AddWithSteamError> {
    let raw = steam::request_app_details(client, appid).await?;
    let app = steam::parse_app_details(&raw.raw, appid)?;
    let game = Game::from(app);
    game.save_as_meta_id().map_err(AddWithSteamError::SaveGame)?;
    Ok(game)
}
