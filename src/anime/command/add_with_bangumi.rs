use std::io;

use super::super::{Anime, SaveAnimeError, index::AnimeIndex};
use crate::source::bangumi::Subject;

#[derive(Debug, thiserror::Error)]
pub enum AddWithBangumiError {
    #[error("bangumi request error")]
    BangumiRequest(#[source] reqwest::Error),
    #[error("save anime error")]
    SaveAnime(#[source] SaveAnimeError),
    #[error("save bangumi subject error")]
    SaveBangumiSubject(#[source] io::Error),
}

pub async fn add_with_bangumi(
    client: &reqwest::Client,
    bangumi_id: i64,
) -> Result<Anime, AddWithBangumiError> {
    let bangumi = Subject::request_with_id(client, bangumi_id)
        .await
        .map_err(AddWithBangumiError::BangumiRequest)?;
    bangumi
        .save()
        .map_err(AddWithBangumiError::SaveBangumiSubject)?;
    let anime: Anime = bangumi.into();
    anime
        .save_as_meta_id()
        .map_err(AddWithBangumiError::SaveAnime)?;
    // 记入索引，让 update-with-bangumi 与 search 立刻能用。索引打不开时只打印
    // 警告：它可以从数据目录重建，不该让添加本身失败。
    if let Some(index) = AnimeIndex::open_or_warn() {
        index.set_anime_index_or_warn(&anime);
        index.flush_or_warn();
    }
    Ok(anime)
}
