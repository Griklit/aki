//! 番剧索引：`root/{anime.path}/.aki/fjall-index/` 下的一个 fjall 数据库。
//!
//! 里面只有几个 keyspace，都是平表，没有倒排、没有分词：
//!
//! | keyspace | 键 | 值 | 用途 |
//! |----------|----|----|------|
//! | `bangumi-id-to-meta-id` | `bangumi_id` | `meta.id` | `update-with-bangumi` 直接定位文件 |
//! | `title-to-meta-id` | 小写化的标题 | `meta.id` | `search` 按标题找番剧 |
//! | `tag-to-meta-id` | 小写化的标签 + `\x1f` + `meta.id` | `meta.id` | `search --tag` 按标签找番剧 |
//!
//! 一条番剧有几个标题（原名 + 各语言翻译）就在 `title-to-meta-id` 里占几个键，有几个
//! 标签就在 `tag-to-meta-id` 里占几个键。标题表是「一个键对一个条目」，而标签天然是
//! 「一个标签对多个条目」，所以标签的键里带上 `meta.id`——查一个标签就是扫一遍它的
//! 前缀，写入也是幂等的，不用「读出来、改一改、写回去」。
//!
//! 索引是随时可以从数据目录重建的缓存：打不开就退化成「没有索引」，调用方回退
//! 到遍历数据文件。

use std::path::Path;

use fjall::{Database, Keyspace, KeyspaceCreateOptions};

use super::super::{Anime, AnimeNamespace};

/// `bangumi_id` → `meta.id` 的 keyspace。
const BANGUMI_ID_TO_META_ID_KEYSPACE: &str = "bangumi-id-to-meta-id";
/// 标题 → `meta.id` 的 keyspace，键是小写化后的标题。
const TITLE_TO_META_ID_KEYSPACE: &str = "title-to-meta-id";
/// 标签 → `meta.id` 的 keyspace，键是小写化后的标签 + [`TAG_SEPARATOR`] + `meta.id`。
const TAG_TO_META_ID_KEYSPACE: &str = "tag-to-meta-id";

/// 标签键里分隔「标签」与 `meta.id` 的字符（U+001F，标签和 `meta.id` 里都不会有它）。
const TAG_SEPARATOR: char = '\u{1f}';

/// 番剧索引：一个 fjall 数据库加几个平表 keyspace。
///
/// 打开数据库会**独占目录**，同一进程里对同一目录只应存在一个实例，因此各命令
/// 都是用完即弃。
pub struct AnimeIndex {
    database: Database,
    bangumi_id_to_meta_id: Keyspace,
    title_to_meta_id: Keyspace,
    tag_to_meta_id: Keyspace,
}

/// [`AnimeIndex::build`] 的返回：各写了多少条。
pub struct BuildResult {
    pub bangumi_id_to_meta_id: usize,
    pub title_to_meta_id: usize,
    pub tag_to_meta_id: usize,
    pub errors: Vec<fjall::Error>,
}

/// `tag-to-meta-id` 的键：小写标签 + 分隔符 + `meta.id`。
fn tag_key(tag: &str, meta_id: &str) -> String {
    format!("{}{TAG_SEPARATOR}{meta_id}", tag.trim().to_lowercase())
}

impl AnimeIndex {
    /// 打开（必要时创建）指定目录下的索引数据库。
    ///
    /// # Errors
    /// 目录建不出来或数据库打不开时返回错误（例如已被另一个进程独占）。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, fjall::Error> {
        let path = path.as_ref();
        std::fs::create_dir_all(path).map_err(fjall::Error::Io)?;
        let database = Database::builder(path).open()?;
        Ok(Self {
            bangumi_id_to_meta_id: database.keyspace(
                BANGUMI_ID_TO_META_ID_KEYSPACE,
                KeyspaceCreateOptions::default,
            )?,
            title_to_meta_id: database
                .keyspace(TITLE_TO_META_ID_KEYSPACE, KeyspaceCreateOptions::default)?,
            tag_to_meta_id: database
                .keyspace(TAG_TO_META_ID_KEYSPACE, KeyspaceCreateOptions::default)?,
            database,
        })
    }

    /// 打开默认位置的索引（[`AnimeNamespace::index_path()`]），失败时打印警告并
    /// 返回 `None`。
    ///
    /// 索引是可重建的缓存，不该因为它打不开就让真正的增删改失败。
    #[must_use]
    pub fn open_or_warn() -> Option<Self> {
        match Self::open(AnimeNamespace::index_path()) {
            Ok(index) => Some(index),
            Err(error) => {
                eprintln!(
                    "[?] failed to open anime index '{}': {error}",
                    AnimeNamespace::index_path().display()
                );
                None
            }
        }
    }

    /// 记下 `bangumi_id` → `meta.id`，写入失败只打印警告。
    pub fn set_bangumi_id_to_meta_id(
        &self,
        bangumi_id: i64,
        meta_id: &str,
    ) -> Result<(), fjall::Error> {
        self.bangumi_id_to_meta_id
            .insert(bangumi_id.to_be_bytes(), meta_id)
    }

    /// 按 `bangumi_id` 查 `meta.id`。
    #[must_use]
    pub fn get_meta_id_of_bangumi_id(&self, bangumi_id: i64) -> Option<String> {
        let value = match self.bangumi_id_to_meta_id.get(bangumi_id.to_be_bytes()) {
            Ok(value) => value?,
            Err(error) => {
                eprintln!("[?] failed to read anime index: {error}");
                return None;
            }
        };
        String::from_utf8(value.to_vec()).ok()
    }

    /// 记下 `标题` → `meta.id`。
    ///
    /// 键统一小写，这样查询时也小写化就能大小写不敏感地匹配。
    pub fn set_title_to_meta_id(&self, title: &str, meta_id: &str) -> Result<(), fjall::Error> {
        if title.trim().is_empty() {
            // fjall 不接受空键；空标题也不值得拿去匹配。
            return Ok(());
        }
        self.title_to_meta_id.insert(title.to_lowercase(), meta_id)
    }

    /// 记下 `标签` → `meta.id`。
    ///
    /// 键是「小写标签 + 分隔符 + `meta.id`」，所以同一个标签下的多个条目各占一个键；
    /// 同一个 `(标签, meta.id)` 重复写只是覆盖，天然幂等。空标签跳过（fjall 不接受空键）。
    pub fn set_tag_to_meta_id(&self, tag: &str, meta_id: &str) -> Result<(), fjall::Error> {
        if tag.trim().is_empty() {
            return Ok(());
        }
        self.tag_to_meta_id.insert(tag_key(tag, meta_id), meta_id)
    }

    /// 某个标签下的全部 `meta.id`（标签大小写不敏感、首尾空白忽略）。
    ///
    /// 扫的是这个标签的键前缀，所以只碰它自己的那一段；读不动的条目跳过。
    #[must_use]
    pub fn meta_ids_of_tag(&self, tag: &str) -> Vec<String> {
        let prefix = format!("{}{TAG_SEPARATOR}", tag.trim().to_lowercase());
        self.tag_to_meta_id
            .prefix(prefix)
            .filter_map(|guard| {
                let (_, value) = guard.into_inner().ok()?;
                String::from_utf8(value.to_vec()).ok()
            })
            .collect()
    }

    /// 标签表里有没有东西。
    ///
    /// 用来区分「索引里真的没有这个标签」和「这份索引是加标签之前建的」——后者该提示
    /// 用户重建索引，而不是让搜索悄悄返回空。
    #[must_use]
    pub fn has_tag_entries(&self) -> bool {
        match self.tag_to_meta_id.len() {
            Ok(len) => len > 0,
            Err(error) => {
                eprintln!("[?] failed to read anime index: {error}");
                false
            }
        }
    }

    pub fn set_anime_index(&self, anime: &Anime) -> Result<(), Vec<(&'static str, fjall::Error)>> {
        let mut errors = Vec::new();
        if let Some(bangumi_id) = anime.bangumi_id {
            let ret = self.set_bangumi_id_to_meta_id(bangumi_id, anime.meta.id.id.as_str());
            if let Err(err) = ret {
                errors.push((BANGUMI_ID_TO_META_ID_KEYSPACE, err));
            }
        }
        for title in anime.all_titles() {
            let ret = self.set_title_to_meta_id(title, anime.meta.id.id.as_str());
            if let Err(err) = ret {
                errors.push((TITLE_TO_META_ID_KEYSPACE, err));
            }
        }
        for tag in &anime.info.tags {
            let ret = self.set_tag_to_meta_id(tag, anime.meta.id.id.as_str());
            if let Err(err) = ret {
                errors.push((TAG_TO_META_ID_KEYSPACE, err));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn set_anime_index_or_warn(&self, anime: &Anime) {
        let ret = self.set_anime_index(anime);
        if let Err(err) = ret {
            for (keyspace, err) in err {
                eprintln!("[?] failed to set anime {keyspace} index: {err}");
            }
        }
    }

    /// 遍历整个 `title-to-meta-id` keyspace，返回 `(标题, meta.id)`。
    pub fn title_entries(&self) -> impl Iterator<Item = (String, String)> {
        self.title_to_meta_id.iter().filter_map(|guard| {
            let (key, value) = guard.into_inner().ok()?;
            let title = String::from_utf8(key.to_vec()).ok()?;
            let meta_id = String::from_utf8(value.to_vec()).ok()?;
            Some((title, meta_id))
        })
    }

    /// 遍历番剧数据目录整体重建索引。
    ///
    /// 会先清空三个 keyspace 再写入，所以删掉的番剧不会残留在索引里；写完落一次
    /// 盘，免得这些写入留在 journal 里让之后每次打开数据库都变慢。
    ///
    /// # Errors
    /// 索引打不开、清空或写入失败时返回错误——这是 `build-index` 显式要求做的事，
    /// 不该只打印一行警告就当作成功。
    pub fn build() -> Result<BuildResult, fjall::Error> {
        let index = Self::open(AnimeNamespace::index_path())?;
        index.bangumi_id_to_meta_id.clear()?;
        index.title_to_meta_id.clear()?;
        index.tag_to_meta_id.clear()?;

        let mut bangumi_ids = 0;
        let mut title_keys = 0;
        let mut tag_keys = 0;
        let mut errors = Vec::new();
        for anime in Anime::iter_all() {
            let meta_id = anime.meta.id.id.as_str();
            if let Some(bangumi_id) = anime.bangumi_id {
                let ret = index.set_bangumi_id_to_meta_id(bangumi_id, meta_id);
                if let Err(ret) = ret {
                    errors.push(ret);
                } else {
                    bangumi_ids += 1;
                }
            }
            for title in anime.all_titles() {
                let ret = index.set_title_to_meta_id(title, meta_id);
                if let Err(ret) = ret {
                    errors.push(ret);
                } else {
                    title_keys += 1;
                }
            }
            for tag in &anime.info.tags {
                let ret = index.set_tag_to_meta_id(tag, meta_id);
                if let Err(ret) = ret {
                    errors.push(ret);
                } else {
                    tag_keys += 1;
                }
            }
        }
        index.flush()?;
        Ok(BuildResult {
            bangumi_id_to_meta_id: bangumi_ids,
            title_to_meta_id: title_keys,
            tag_to_meta_id: tag_keys,
            errors,
        })
    }

    /// 把批量写入落进磁盘表并 fsync。
    ///
    /// # Errors
    /// 落盘失败时返回错误。
    pub fn flush(&self) -> Result<(), fjall::Error> {
        self.bangumi_id_to_meta_id.rotate_memtable_and_wait()?;
        self.title_to_meta_id.rotate_memtable_and_wait()?;
        self.tag_to_meta_id.rotate_memtable_and_wait()?;
        self.database.persist(fjall::PersistMode::SyncAll)
    }

    pub fn flush_or_warn(&self) {
        let ret = self.flush();
        if let Err(err) = ret {
            eprintln!("[?] failed to flush anime index: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::anime::{AnimeInfo, AnimeState};
    use crate::{Language, Meta};

    /// 临时索引目录，Drop 时删掉。
    struct TempIndex(std::path::PathBuf);

    impl TempIndex {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("aki-tag-index-{}", uuid::Uuid::now_v7()));
            Self(path)
        }
    }

    impl Drop for TempIndex {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn anime(meta_id: &str, tags: &[&str]) -> Anime {
        Anime::new(
            Meta::with_id(meta_id.to_string(), None),
            AnimeInfo {
                title: format!("{meta_id} title"),
                summary: None,
                platform: None,
                air_date: None,
                season: None,
                episodes: None,
                tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            },
            BTreeMap::<Language, crate::anime::AnimeInfoInternationalization>::new(),
            AnimeState::default(),
            None,
        )
    }

    fn meta_ids(index: &AnimeIndex, tag: &str) -> Vec<String> {
        let mut ids = index.meta_ids_of_tag(tag);
        ids.sort();
        ids
    }

    /// 一个标签对应多个条目，大小写与首尾空白都不影响查询。
    #[test]
    fn tag_lookup_is_case_insensitive_and_multi_valued() {
        let temp = TempIndex::new();
        let index = AnimeIndex::open(&temp.0).unwrap();
        assert!(!index.has_tag_entries(), "新库里应当没有标签");

        index
            .set_anime_index(&anime("[2020-01-01] A", &["奇幻", "TV"]))
            .unwrap();
        index
            .set_anime_index(&anime("[2021-01-01] B", &["奇幻", "SHAFT"]))
            .unwrap();
        index.flush().unwrap();

        assert_eq!(
            meta_ids(&index, "奇幻"),
            vec!["[2020-01-01] A", "[2021-01-01] B"]
        );
        assert_eq!(meta_ids(&index, "  TV "), vec!["[2020-01-01] A"]);
        assert_eq!(meta_ids(&index, "tv"), vec!["[2020-01-01] A"]);
        assert_eq!(meta_ids(&index, "SHAFT"), vec!["[2021-01-01] B"]);
        assert!(meta_ids(&index, "不存在的标签").is_empty());
        assert!(index.has_tag_entries());

        // 前缀不能串台：`TV` 不该被 `TV动画` 之类捞出来
        index
            .set_anime_index(&anime("[2022-01-01] C", &["TV动画"]))
            .unwrap();
        assert_eq!(meta_ids(&index, "TV"), vec!["[2020-01-01] A"]);
    }

    /// 重复写同一个 `(标签, meta.id)` 只是覆盖，不会出现两条。
    #[test]
    fn setting_the_same_tag_twice_keeps_one_entry() {
        let temp = TempIndex::new();
        let index = AnimeIndex::open(&temp.0).unwrap();
        let anime = anime("[2020-01-01] A", &["奇幻"]);
        index.set_anime_index(&anime).unwrap();
        index.set_anime_index(&anime).unwrap();
        index.flush().unwrap();
        assert_eq!(meta_ids(&index, "奇幻"), vec!["[2020-01-01] A"]);
    }

    /// 空标签不进索引（fjall 不接受空键），没有标签的条目也不会写出任何标签键。
    #[test]
    fn empty_tags_are_skipped() {
        let temp = TempIndex::new();
        let index = AnimeIndex::open(&temp.0).unwrap();
        index
            .set_anime_index(&anime("[2020-01-01] A", &["  ", ""]))
            .unwrap();
        assert!(index.meta_ids_of_tag("").is_empty());
        assert!(!index.has_tag_entries());
    }
}
