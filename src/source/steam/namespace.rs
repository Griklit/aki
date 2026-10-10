use crate::Namespace;

/// Steam 原始响应的命名空间：`[source.steam] path`（默认 `database/source/steam`）。
///
/// 与 Bangumi 存 `Subject` 原样响应同理——接口返回的 JSON 一律先存下来，
/// 解析逻辑改了就离线重跑，不用再打接口。
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Copy)]
pub struct SteamNamespace;

impl Namespace for SteamNamespace {
    fn new() -> Self {
        SteamNamespace
    }

    fn kind() -> &'static str {
        "steam"
    }
}
