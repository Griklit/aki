use crate::config::{CONFIG, ConfigSteam};
use crate::utils::text::truncate;
use serde::de::DeserializeOwned;

/// 请求 Steam 时产生的错误。
#[derive(Debug, thiserror::Error)]
pub enum SteamRequestError {
    #[error("http request error")]
    Request(#[source] reqwest::Error),
    #[error("steam returned status {status} for '{url}': {body}")]
    Status {
        status: reqwest::StatusCode,
        url: String,
        body: String,
    },
    #[error("steam response parse error for '{url}': {source}; response starts with: {body}")]
    Parse {
        #[source]
        source: serde_json::Error,
        url: String,
        body: String,
    },
    #[error("steam response for '{url}' was not valid UTF-8")]
    InvalidUtf8 {
        #[source]
        source: reqwest::Error,
        url: String,
    },
    #[error(
        "steam response did not contain appid {appid} (removed, region-locked, or stored with another appid)"
    )]
    AppNotFound { appid: u32 },
    #[error("steam returned appid {actual} but appid {expected} was requested")]
    ResponseAppIdMismatch { expected: u32, actual: u32 },
    #[error("config '{name}' is empty, set it in database/aki.toml")]
    MissingConfig { name: &'static str },
    #[error("save/load steam raw data error")]
    RawStore(#[source] std::io::Error),
}

/// 一个 Steam 接口：路径、查询参数、响应类型、以及响应体怎么变成结果。
pub trait Endpoint {
    /// 解析出来的结果类型。
    type Response;

    /// 拼在 base 后面的路径，如 `/ISteamUser/GetPlayerSummaries/v2/`。
    fn path(&self) -> String;

    /// 查询参数。
    fn params(&self) -> Vec<(&'static str, String)>;

    /// `true` 用 Web API（`api.steampowered.com`），`false` 用商店接口。
    fn use_web_api(&self) -> bool;

    /// 需要密钥的接口返回 `true`。
    fn needs_api_key(&self) -> bool {
        false
    }

    /// 要落盘的原始响应，默认不落盘。
    fn raw_body(&self, body: &str) -> Result<Option<String>, SteamRequestError> {
        let _ = body;
        Ok(None)
    }

    /// 发请求前的参数校验，默认不检查。
    fn validate(&self) -> Result<(), SteamRequestError> {
        Ok(())
    }

    fn base(&self, steam_config: &ConfigSteam) -> String {
        if self.use_web_api() {
            steam_config.api.clone()
        } else {
            steam_config.store_api.clone()
        }
    }

    /// 响应体 → 结果。`url` 留空即可，[`request`] 会补上。
    fn parse(&self, body: &str) -> Result<Self::Response, SteamRequestError>;
}

/// 请求一个 [`Endpoint`]，带上配置里的密钥、语言与地区。
///
/// # Errors
/// 网络失败、非 2xx 状态码、响应不是合法 JSON、或 endpoint 自己的校验失败时返回错误。
pub async fn request<E: Endpoint>(
    client: &reqwest::Client,
    endpoint: &E,
) -> Result<E::Response, SteamRequestError> {
    let (_, response) = request_with_raw(client, endpoint).await?;
    Ok(response)
}

/// 同 [`request`]，但同时给出要落盘的原始响应（未要求落盘时为 `None`）。
///
/// # Errors
/// 同 [`request`]，另外包含落盘文本校验失败的情况。
pub async fn request_with_raw<E: Endpoint>(
    client: &reqwest::Client,
    endpoint: &E,
) -> Result<(Option<String>, E::Response), SteamRequestError> {
    endpoint.validate()?;
    let steam_config = &CONFIG.steam;
    let mut params = endpoint.params();
    if endpoint.needs_api_key() {
        if steam_config.api_key.is_empty() {
            return Err(SteamRequestError::MissingConfig {
                name: "steam.api_key",
            });
        }
        params.push(("key", steam_config.api_key.clone()));
    }
    let url = format!("{}{}", endpoint.base(steam_config), endpoint.path());
    let response = client
        .get(&url)
        .query(&params)
        .header(reqwest::header::USER_AGENT, "Griklit/aki")
        .send()
        .await
        .map_err(SteamRequestError::Request)?;
    let status = response.status();
    let body = response.text().await.map_err(|source| {
        if status.is_success() {
            SteamRequestError::InvalidUtf8 {
                source,
                url: url.clone(),
            }
        } else {
            SteamRequestError::Request(source)
        }
    })?;
    if !status.is_success() {
        return Err(SteamRequestError::Status {
            status,
            url,
            body: truncate(&body, 200),
        });
    }
    let parsed = endpoint.parse(&body).map_err(|e| match e {
        SteamRequestError::Parse { source, .. } => SteamRequestError::Parse {
            source,
            url,
            body: truncate(&body, 200),
        },
        other => other,
    })?;
    match endpoint.raw_body(&body)? {
        Some(raw) => Ok((Some(raw), parsed)),
        None => Ok((None, parsed)),
    }
}

/// 解析成 `T`，并把 JSON 错误统一包装。
pub fn parse_json<T: DeserializeOwned>(body: &str) -> Result<T, SteamRequestError> {
    serde_json::from_str(body).map_err(|source| SteamRequestError::Parse {
        source,
        url: String::new(),
        body: truncate(body, 200),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_reports_error() {
        assert!(parse_json::<Vec<u32>>("{").is_err());
    }
}
