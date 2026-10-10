//! 番剧索引：`{anime_data_path}/.aki/fjall-index/` 下的一个 fjall 数据库。
//!
//! 里面只有两个 keyspace，每个都是「键 → 一个 `meta.id`」的平表，没有倒排、
//! 没有分词：
//!
//! | keyspace | 键 | 值 | 用途 |
//! |----------|----|----|------|
//! | `bangumi-id` | `bangumi_id` | `meta.id` | `update-with-bangumi` 直接定位文件 |
//! | `title` | 小写化的标题 | `meta.id` | `search` 按标题找番剧 |
//!
//! 一条番剧有几个标题（原名 + 各语言翻译）就在 `title` 里占几个键。数据量很小
//! （几百条），所以搜索就是把整张表扫一遍做字符串匹配，比维护倒排简单得多。
//!
//! 索引是随时可以从数据目录重建的缓存：打不开就退化成「没有索引」，调用方回退
//! 到遍历数据文件。

use std::path::Path;

use fjall::{Database, Keyspace, KeyspaceCreateOptions};

use super::super::Anime;
use crate::CONFIG;

/// `bangumi_id` → `meta.id` 的 keyspace。
const BANGUMI_ID_TO_META_ID_KEYSPACE: &str = "bangumi-id-to-meta-id";
/// 标题 → `meta.id` 的 keyspace，键是小写化后的标题。
const TITLE_TO_META_ID_KEYSPACE: &str = "title-to-meta-id";

/// 番剧索引：一个 fjall 数据库加两个平表 keyspace。
///
/// 打开数据库会**独占目录**，同一进程里对同一目录只应存在一个实例，因此各命令
/// 都是用完即弃。
pub struct AnimeIndex {
    database: Database,
    bangumi_id_to_meta_id: Keyspace,
    title_to_meta_id: Keyspace,
}

/// [`Index::build`] 的返回：各写了多少条。
pub struct BuildResult {
    pub bangumi_id_to_meta_id: usize,
    pub title_to_meta_id: usize,
    pub errors: Vec<fjall::Error>,
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
            database,
        })
    }

    /// 打开默认位置的索引（`{anime_data_path}/.aki/fjall-index/`），失败时打印
    /// 警告并返回 `None`。
    ///
    /// 索引是可重建的缓存，不该因为它打不开就让真正的增删改失败。
    #[must_use]
    pub fn open_or_warn() -> Option<Self> {
        let path = CONFIG.anime_fjall_index_path();
        match Self::open(path) {
            Ok(index) => Some(index),
            Err(error) => {
                eprintln!(
                    "[?] failed to open anime index '{}': {error}",
                    path.display()
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
    /// 会先清空两个 keyspace 再写入，所以删掉的番剧不会残留在索引里；写完落一次
    /// 盘，免得这些写入留在 journal 里让之后每次打开数据库都变慢。
    ///
    /// # Errors
    /// 索引打不开、清空或写入失败时返回错误——这是 `build-index` 显式要求做的事，
    /// 不该只打印一行警告就当作成功。
    pub fn build() -> Result<BuildResult, fjall::Error> {
        let index = Self::open(CONFIG.anime_fjall_index_path())?;
        index.bangumi_id_to_meta_id.clear()?;
        index.title_to_meta_id.clear()?;

        let mut bangumi_ids = 0;
        let mut title_keys = 0;
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
        }
        index.flush()?;
        Ok(BuildResult {
            bangumi_id_to_meta_id: bangumi_ids,
            title_to_meta_id: title_keys,
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
        self.database.persist(fjall::PersistMode::SyncAll)
    }

    pub fn flush_or_warn(&self) {
        let ret = self.flush();
        if let Err(err) = ret {
            eprintln!("[?] failed to flush anime index: {err}");
        }
    }
}
