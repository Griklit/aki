use std::collections::HashMap;
use std::io;

use lindera::dictionary::{DictionaryKind, load_embedded_dictionary};
use lindera::error::LinderaError;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera_tantivy::tokenizer::LinderaTokenizer;
use tantivy::collector::TopDocs;
use tantivy::query::{QueryParser, QueryParserError};
use tantivy::schema::{
    Field, IndexRecordOption, STORED, Schema, TEXT, TextFieldIndexing, TextOptions, Value,
};
use tantivy::{Index, IndexWriter, TantivyDocument, TantivyError, doc};
use tantivy_jieba::JiebaTokenizer;

use super::super::{Anime, AnimeContext, LoadAnimeError};
use crate::{CONFIG, Language, MetaId};

/// Field names used in the tantivy index schema.
const META_ID_FIELD: &str = "meta_id";
const TITLE_FIELD_LABEL: &str = "title";
const SUMMARY_FIELD_LABEL: &str = "summary";

pub struct AnimeTantivyIndexSchema {
    schema: Schema,
    meta_id_field: Field,
    title_fields: HashMap<Language, Field>,
    summary_fields: HashMap<Language, Field>,
}

impl AnimeTantivyIndexSchema {
    #[must_use]
    pub fn new() -> Self {
        let mut builder = Schema::builder();
        let meta_id_field = builder.add_text_field(META_ID_FIELD, TEXT | STORED);
        let zh_field_options = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(Language::ZhCn.kind())
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );
        let ja_field_options = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(Language::JaJp.kind())
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );
        let zh_title_field = builder.add_text_field(
            format!("{}-{}", TITLE_FIELD_LABEL, Language::ZhCn).as_str(),
            zh_field_options.clone().set_stored(),
        );
        let ja_title_field = builder.add_text_field(
            format!("{}-{}", TITLE_FIELD_LABEL, Language::JaJp).as_str(),
            ja_field_options.clone().set_stored(),
        );
        let en_title_field = builder.add_text_field(
            format!("{}-{}", TITLE_FIELD_LABEL, Language::EnUs).as_str(),
            TEXT | STORED,
        );
        let zh_summary_field = builder.add_text_field(
            format!("{}-{}", SUMMARY_FIELD_LABEL, Language::ZhCn).as_str(),
            zh_field_options.clone(),
        );
        let ja_summary_field = builder.add_text_field(
            format!("{}-{}", SUMMARY_FIELD_LABEL, Language::JaJp).as_str(),
            ja_field_options.clone(),
        );
        let en_summary_field = builder.add_text_field(
            format!("{}-{}", SUMMARY_FIELD_LABEL, Language::EnUs).as_str(),
            TEXT,
        );
        Self {
            schema: builder.build(),
            meta_id_field,
            title_fields: HashMap::from([
                (Language::ZhCn, zh_title_field),
                (Language::JaJp, ja_title_field),
                (Language::EnUs, en_title_field),
            ]),
            summary_fields: HashMap::from([
                (Language::ZhCn, zh_summary_field),
                (Language::JaJp, ja_summary_field),
                (Language::EnUs, en_summary_field),
            ]),
        }
    }
}

impl Default for AnimeTantivyIndexSchema {
    fn default() -> Self {
        Self::new()
    }
}

fn jieba_tokenizer() -> JiebaTokenizer {
    JiebaTokenizer::new()
}

fn lindera_tokenizer() -> Result<LinderaTokenizer, LinderaError> {
    let mode = Mode::Normal;
    let dictionary = load_embedded_dictionary(DictionaryKind::IPADIC)?;
    let segmenter = Segmenter::new(mode, dictionary, None);
    let tokenizer = LinderaTokenizer::from_segmenter(segmenter);
    Ok(tokenizer)
}

#[derive(Debug, thiserror::Error)]
pub enum BuildTantivyIndexError {
    #[error("failed to remove old index directory")]
    RemoveOldIndexDir(#[source] io::Error),
    #[error("failed to create index directory")]
    CreateIndexDir(#[source] io::Error),
    #[error("failed to create index")]
    CreateIndex(#[source] TantivyError),
    #[error("failed to create index writer")]
    CreateWriter(#[source] TantivyError),
    #[error("failed to add document")]
    AddDocument(#[source] TantivyError),
    #[error("failed to commit index writer")]
    Commit(#[source] TantivyError),
    #[error("failed to wait for merging threads")]
    WaitMerge(#[source] TantivyError),
    #[error("failed to load lindera tokenizer dictionary")]
    LinderaTokenizerDictionaryLoadFailed(#[source] LinderaError),
}

pub fn build_tantivy_index(context: &AnimeContext) -> Result<usize, BuildTantivyIndexError> {
    let schema = context.tantivy_schema();
    let index_dir = CONFIG.anime_tantivy_index_path();
    if index_dir.exists() {
        std::fs::remove_dir_all(&index_dir).map_err(BuildTantivyIndexError::RemoveOldIndexDir)?;
    }
    std::fs::create_dir_all(&index_dir).map_err(BuildTantivyIndexError::CreateIndexDir)?;
    let index = Index::create_in_dir(&index_dir, schema.schema.clone())
        .map_err(BuildTantivyIndexError::CreateIndex)?;
    index
        .tokenizers()
        .register(Language::ZhCn.kind(), jieba_tokenizer());
    index.tokenizers().register(
        Language::JaJp.kind(),
        lindera_tokenizer()
            .map_err(BuildTantivyIndexError::LinderaTokenizerDictionaryLoadFailed)?,
    );
    let mut writer: IndexWriter = index
        .writer(50_000_000)
        .map_err(BuildTantivyIndexError::CreateWriter)?;
    let mut count: usize = 0;
    for anime in Anime::iter_all() {
        let meta_id = anime.meta.id.id.clone();
        let mut document = doc!(schema.meta_id_field => meta_id);
        for (lang, title) in anime.internationalization_titles() {
            let Some(field) = schema.title_fields.get(&lang) else {
                continue;
            };
            document.add_text(*field, title);
        }
        for (lang, summary) in anime.internationalization_summaries() {
            let Some(field) = schema.summary_fields.get(&lang) else {
                continue;
            };
            document.add_text(*field, summary);
        }
        writer
            .add_document(document)
            .map_err(BuildTantivyIndexError::AddDocument)?;
        count += 1;
    }
    writer.commit().map_err(BuildTantivyIndexError::Commit)?;
    writer
        .wait_merging_threads()
        .map_err(BuildTantivyIndexError::WaitMerge)?;
    Ok(count)
}

#[derive(Debug, thiserror::Error)]
pub enum GetTantivyIndexError {
    #[error("failed to open index")]
    Open(#[source] TantivyError),
    #[error("create index dir")]
    CreateIndexDir(#[source] io::Error),
    #[error("failed to create index")]
    CreateIndex(#[source] TantivyError),
    #[error("lindera tokenizer dictionary load failed")]
    GetLinderaTOkenizer(#[source] LinderaError),
}

pub fn get_tantivy_index() -> Result<Index, GetTantivyIndexError> {
    type E = GetTantivyIndexError;
    let index_dir = CONFIG.anime_tantivy_index_path();
    let index = if index_dir.exists() {
        Index::open_in_dir(&index_dir).map_err(E::Open)?
    } else {
        std::fs::create_dir_all(&index_dir).map_err(E::CreateIndexDir)?;
        let schema = AnimeTantivyIndexSchema::new().schema;
        Index::create_in_dir(&index_dir, schema).map_err(E::CreateIndex)?
    };
    index
        .tokenizers()
        .register(Language::ZhCn.kind(), jieba_tokenizer());
    index.tokenizers().register(
        Language::JaJp.kind(),
        lindera_tokenizer().map_err(E::GetLinderaTOkenizer)?,
    );
    Ok(index)
}

#[derive(Debug, thiserror::Error)]
pub enum SearchTantivyError {
    #[error("failed to get reader")]
    QueryParse(#[source] QueryParserError),
    #[error("failed to search")]
    Search(#[source] TantivyError),
    #[error("failed to fetch document")]
    FetchDocument(#[source] TantivyError),
    #[error("index invalid")]
    IndexInvalid,
    #[error("failed to load anime")]
    LoadAnime(#[source] LoadAnimeError),
}

pub fn search_tantivy(
    context: &AnimeContext,
    query: &str,
    limit: usize,
) -> Result<Vec<Anime>, SearchTantivyError> {
    let schema = context.tantivy_schema();
    let searcher = context.tantivy_reader().searcher();
    let query_parser = QueryParser::for_index(
        searcher.index(),
        schema.title_fields.values().copied().collect::<Vec<_>>(),
    );
    let query = query_parser
        .parse_query(query)
        .map_err(SearchTantivyError::QueryParse)?;
    let top_docs = searcher
        .search(&query, &TopDocs::with_limit(limit).order_by_score())
        .map_err(SearchTantivyError::Search)?;
    let mut meta_ids = Vec::with_capacity(limit);
    for (_score, doc_address) in top_docs {
        let doc = searcher
            .doc::<TantivyDocument>(doc_address)
            .map_err(SearchTantivyError::FetchDocument)?;
        let meta_id = doc
            .get_first(schema.meta_id_field)
            .and_then(|v| v.as_str())
            .ok_or(SearchTantivyError::IndexInvalid)?;
        meta_ids.push(meta_id.to_string());
    }
    let animes = meta_ids
        .into_iter()
        .map(|id| Anime::load_with_meta_id(&MetaId::with_id(id)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(SearchTantivyError::LoadAnime)?;
    Ok(animes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::ANIME_CONTEXT;

    #[test]
    fn test_search_tantivy() {
        ANIME_CONTEXT.init_tantivy_reader().unwrap();
        let animes = search_tantivy(&ANIME_CONTEXT, "物语", 10).unwrap();
        for anime in animes {
            let anime = anime.to_string();
            dbg!(anime);
        }
    }
}
