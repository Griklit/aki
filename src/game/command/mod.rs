//! 游戏相关命令。

mod add_with_steam;
mod sync_owned_games;

pub use add_with_steam::{AddWithSteamError, add_with_steam};
pub use sync_owned_games::{
    REQUEST_INTERVAL, SyncOwnedGamesError, SyncRecord, SyncResult, load_owned_games_raw,
    owned_games_path, sync_owned_games,
};
