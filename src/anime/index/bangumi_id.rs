use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use super::super::Anime;
use crate::CONFIG;

/// `bangumi_id` → `Anime::meta.id` 的简单索引。
///
/// 存放在 `{anime_data_path}/.aki/bangumi-id-index.json`，用于让
/// `update-with-bangumi` 直接定位番剧，而不必遍历整个数据目录。
///
/// 它是一份可以从数据目录重建的缓存，因此读写失败只会打印警告，不会让
/// 调用方真正要做的数据操作失败；下一次成功的写入会自动修复它。
pub struct BangumiIdIndex {
    path: PathBuf,
    entries: BTreeMap<i64, String>,
}

#[derive(Debug, thiserror::Error)]
enum IndexError {
    #[error("create index directory error")]
    CreateDir(#[source] io::Error),
    #[error("io error")]
    Io(#[source] io::Error),
    #[error("json deserialize error")]
    JsonDeserialize(#[source] serde_json::Error),
    #[error("json serialize error")]
    JsonSerialize(#[source] serde_json::Error),
}

impl BangumiIdIndex {
    /// 从 `{anime_data_path}/.aki/bangumi-id-index.json` 载入索引。
    ///
    /// 索引文件不存在时得到空索引；文件损坏时打印警告并同样按空索引处理。
    #[must_use]
    pub fn load() -> Self {
        Self::load_at(CONFIG.anime_bangumi_id_index_path())
    }

    /// 从指定路径载入索引。
    #[must_use]
    pub fn load_at(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        match read_entries(&path) {
            Ok(entries) => Self { path, entries },
            Err(error) => {
                eprintln!(
                    "Warning: failed to read bangumi id index '{}': {error}",
                    path.display()
                );
                Self {
                    path,
                    entries: BTreeMap::new(),
                }
            }
        }
    }

    /// 查询 `bangumi_id` 对应的 `Anime::meta.id`。
    #[must_use]
    pub fn get(&self, bangumi_id: i64) -> Option<&str> {
        self.entries.get(&bangumi_id).map(String::as_str)
    }

    /// 记录映射并立即落盘，写入失败只会打印警告。
    pub fn set(&mut self, bangumi_id: i64, meta_id: &str) {
        self.entries.insert(bangumi_id, meta_id.to_string());
        write_or_warn(&self.path, &self.entries);
    }
}

/// 遍历番剧数据目录整体重建索引，返回写入的映射数量。
///
/// 结果会整体覆盖原索引文件，因此被删掉的番剧不会残留在索引里。
/// 与 [`BangumiIdIndex::set`] 一样，写入失败只会打印警告。
pub fn build_bangumi_id_index() -> usize {
    let entries = Anime::iter_all()
        .filter_map(|anime| Some((anime.bangumi_id?, anime.meta.id.id)))
        .collect::<BTreeMap<_, _>>();
    write_or_warn(CONFIG.anime_bangumi_id_index_path(), &entries);
    entries.len()
}

fn read_entries(path: &Path) -> Result<BTreeMap<i64, String>, IndexError> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let file = File::open(path).map_err(IndexError::Io)?;
    serde_json::from_reader(file).map_err(IndexError::JsonDeserialize)
}

/// 写入索引文件，失败只打印警告。
fn write_or_warn(path: &Path, entries: &BTreeMap<i64, String>) {
    if let Err(error) = write_entries(path, entries) {
        eprintln!(
            "Warning: failed to write bangumi id index '{}': {error}",
            path.display()
        );
    }
}

fn write_entries(path: &Path, entries: &BTreeMap<i64, String>) -> Result<(), IndexError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(IndexError::CreateDir)?;
    }
    let file = File::create(path).map_err(IndexError::Io)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, entries).map_err(IndexError::JsonSerialize)?;
    writer.flush().map_err(IndexError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_index_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("aki-{name}-{}", uuid::Uuid::now_v7()))
            .join("bangumi-id-index.json")
    }

    fn remove_temp_index(path: &Path) {
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn set_and_load_roundtrip() {
        let path = temp_index_path("bangumi-id-index");
        let mut index = BangumiIdIndex::load_at(&path);
        assert_eq!(index.get(265), None);

        index.set(265, "[1995-10-04] 新世紀エヴァンゲリオン");
        index.set(
            475_354,
            "[2024-07-06] 〈物語〉シリーズ オフ&モンスターシーズン",
        );

        // 重新载入，映射应当已经落盘。
        let reloaded = BangumiIdIndex::load_at(&path);
        assert_eq!(
            reloaded.get(265),
            Some("[1995-10-04] 新世紀エヴァンゲリオン")
        );
        assert_eq!(
            reloaded.get(475_354),
            Some("[2024-07-06] 〈物語〉シリーズ オフ&モンスターシーズン")
        );
        assert_eq!(reloaded.get(999), None);

        remove_temp_index(&path);
    }

    #[test]
    fn set_overwrites_existing_mapping() {
        let path = temp_index_path("bangumi-id-index-overwrite");
        let mut index = BangumiIdIndex::load_at(&path);
        index.set(265, "[1995-10-04] 新世紀エヴァンゲリオン");
        index.set(265, "[1999-01-01] renamed");

        assert_eq!(
            BangumiIdIndex::load_at(&path).get(265),
            Some("[1999-01-01] renamed")
        );

        remove_temp_index(&path);
    }

    #[test]
    fn missing_and_broken_index_are_treated_as_empty() {
        let path = temp_index_path("bangumi-id-index-broken");
        // 索引文件不存在。
        assert_eq!(BangumiIdIndex::load_at(&path).get(265), None);

        // 索引文件损坏时按空索引处理，并能被下一次写入修复。
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not a json object").unwrap();
        let mut index = BangumiIdIndex::load_at(&path);
        assert_eq!(index.get(265), None);
        index.set(265, "[1995-10-04] 新世紀エヴァンゲリオン");
        assert_eq!(
            BangumiIdIndex::load_at(&path).get(265),
            Some("[1995-10-04] 新世紀エヴァンゲリオン")
        );

        remove_temp_index(&path);
    }
}
