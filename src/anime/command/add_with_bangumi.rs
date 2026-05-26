use super::super::{Anime, SaveAnimeError};
use crate::bangumi::Subject;

#[derive(Debug, thiserror::Error)]
pub enum AddWithBangumiError {
    #[error("bangumi request error")]
    BangumiRequest(#[source] reqwest::Error),
    #[error("save anime error")]
    Save(#[source] SaveAnimeError),
}

pub async fn add_with_bangumi(
    client: &reqwest::Client,
    bangumi_id: i64,
) -> Result<Anime, AddWithBangumiError> {
    let bangumi = Subject::request_with_id(client, bangumi_id)
        .await
        .map_err(AddWithBangumiError::BangumiRequest)?;
    let anime: Anime = bangumi.into();
    anime.save_as_meta_id().map_err(AddWithBangumiError::Save)?;
    Ok(anime)
}
