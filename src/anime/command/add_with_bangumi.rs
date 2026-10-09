use std::io;

use super::super::{Anime, SaveAnimeError, index::BangumiIdIndex};
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
    // 记入 bangumi-id 索引，让 update-with-bangumi 可以直接定位这条番剧。
    let mut index = BangumiIdIndex::load();
    index.set(bangumi_id, &anime.meta.id.id);
    Ok(anime)
}
