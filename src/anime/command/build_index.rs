use super::super::{ANIME_CONTEXT, index};

#[derive(Debug, thiserror::Error)]
pub enum BuildIndexErropr {
    #[error("build tantivy index error")]
    Tantivy(#[source] index::BuildTantivyIndexError),
}

pub struct BuildIndexResult {
    /// 全文索引写入的番剧数量
    pub animes: usize,
    /// bangumi-id 索引写入的映射数量
    pub bangumi_ids: usize,
}

pub fn build_index() -> Result<BuildIndexResult, BuildIndexErropr> {
    // bangumi-id 索引很便宜，先建好，这样即使随后的全文索引失败它也已经可用。
    let bangumi_ids = index::build_bangumi_id_index();
    let animes = index::build_tantivy_index(&ANIME_CONTEXT).map_err(BuildIndexErropr::Tantivy)?;
    Ok(BuildIndexResult {
        animes,
        bangumi_ids,
    })
}
