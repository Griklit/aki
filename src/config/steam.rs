use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_steam_path() -> PathBuf {
    PathBuf::from("source/steam")
}

fn default_steam_language() -> String {
    "schinese".to_string()
}

fn default_steam_country() -> String {
    "cn".to_string()
}

fn default_steam_api() -> String {
    "https://api.steampowered.com".to_string()
}

fn default_steam_store_api() -> String {
    "https://store.steampowered.com".to_string()
}

/// `[steam]`：Steam 接口配置与原始数据的落盘位置。
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigSteam {
    /// 相对于 [`super::Config::root`] 的目录，原始响应就落在它下面。
    #[serde(default = "default_steam_path")]
    pub path: PathBuf,
    /// Steam Web API 密钥。留空则不发送，需要密钥的接口会直接报错。
    #[serde(default)]
    pub api_key: String,
    /// SteamID64，用于拉取「已拥有游戏」「最近游玩」等玩家数据。
    #[serde(default)]
    pub steam_id: Option<String>,
    /// 商店接口的响应语言，如 `schinese`。
    #[serde(default = "default_steam_language")]
    pub language: String,
    /// 商店接口的地区，决定价格与分区，如 `cn`。
    #[serde(default = "default_steam_country")]
    pub country: String,
    /// Web API 的 base 地址。
    #[serde(default = "default_steam_api")]
    pub api: String,
    /// 商店接口的 base 地址。
    #[serde(default = "default_steam_store_api")]
    pub store_api: String,
}

impl Default for ConfigSteam {
    fn default() -> Self {
        Self {
            path: default_steam_path(),
            api_key: String::new(),
            steam_id: None,
            language: default_steam_language(),
            country: default_steam_country(),
            api: default_steam_api(),
            store_api: default_steam_store_api(),
        }
    }
}
