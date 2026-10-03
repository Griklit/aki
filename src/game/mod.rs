//! 游戏领域模型，与 `anime` 模块同构。
//!
//! 与番剧的差别主要有三处：
//! - 发售日期粒度不一（只有年份、季度甚至「即将推出」），见 [`GameRelease`]
//! - 游玩时长以**秒**记录，成就进度用「已解锁/总数」（如 `23/45`）
//! - 数据来源可能不止一个，来源 id 分开存放（`steam_app_id` / `bangumi_id`）

pub mod command;
mod core;
mod info;
mod namespace;
// mod score; 评分模型还没定，先整个注释掉
pub mod state;
pub mod steam;

pub use core::{Game, LoadGameError, SaveGameError};
pub use info::{
    ExternalScore, GameInfo, GameInfoInternationalization, GamePlatform, GameRelease,
};
pub use namespace::GameNamespace;
pub use state::{GameAchievements, GamePlayStatus, GameState};
