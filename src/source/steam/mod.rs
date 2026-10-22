//! Steam 接口集成：原始的接口调用、响应模型与原始数据落盘。
//!
//! 这里只处理「Steam 返回了什么」；领域模型（[`crate::game::Game`]）的转换在 [`crate::game::steam`] 里。

pub mod date;
pub mod endpoints;
pub mod http;
pub mod model;
mod namespace;
pub mod store;

pub use endpoints::{
    AppDetailsEndpoint, CurrentPlayersEndpoint, OwnedGamesEndpoint, PlayerSummariesEndpoint,
};
pub use http::{Endpoint, SteamRequestError, parse_json, request, request_with_raw};
pub use model::{AppDetails, AppType, OwnedGame, PlayerSummary};
pub use namespace::SteamNamespace;
pub use store::SteamRawStore;
