use super::super::{Anime, SaveAnimeError, index};
use crate::source::bangumi::{LoadOrRequestError, Subject};

#[derive(Debug, thiserror::Error)]
pub enum AddWithBangumiError {
    /// 取条目失败：请求接口或把响应落盘出错（缓存读不出来只会退回请求并警告）。
    #[error("load or request bangumi subject error")]
    FetchSubject(#[source] LoadOrRequestError),
    #[error("save anime error")]
    SaveAnime(#[source] SaveAnimeError),
}

/// 按 Bangumi ID 添加番剧。
///
/// 条目数据优先用本地已经存过的原始响应（见 [`Subject::load_or_request_with_id`]），
/// `refresh` 为 `true` 时忽略缓存、重新请求。
pub async fn add_with_bangumi(
    client: &reqwest::Client,
    bangumi_id: i64,
    refresh: bool,
) -> Result<Anime, AddWithBangumiError> {
    let bangumi = Subject::load_or_request_with_id(client, bangumi_id, refresh)
        .await
        .map_err(AddWithBangumiError::FetchSubject)?;
    let anime: Anime = bangumi.into();
    anime
        .save_as_meta_id()
        .map_err(AddWithBangumiError::SaveAnime)?;
    // 记入索引，让 update-with-bangumi 与 search 立刻能用；索引重建失败只打印警告。
    index::set_anime_index_or_warn(&anime).await;
    Ok(anime)
}
