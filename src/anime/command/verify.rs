use std::path::PathBuf;
use std::{fs, io};

use super::super::{Anime, LoadAnimeError};
use crate::CONFIG;

#[derive(Debug, thiserror::Error)]
pub enum VerifyAnimeFileError {
    #[error("load anime error")]
    LoadAnimeError(#[from] LoadAnimeError),
}

#[derive(Debug)]
pub struct InvaildAnimeFile {
    pub path: PathBuf,
    pub error: VerifyAnimeFileError,
}

pub fn verify() -> Result<(usize, Vec<InvaildAnimeFile>), io::Error> {
    let mut errors = Vec::new();
    let file_iter = fs::read_dir(CONFIG.anime_data_path())?
        .into_iter()
        .flatten();
    let mut count = 0;
    for entry in file_iter {
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "json") {
            continue;
        }
        count += 1;
        let anime = Anime::load(&path);
        if let Err(e) = anime {
            errors.push(InvaildAnimeFile {
                path,
                error: e.into(),
            });
            continue;
        }
    }
    Ok((count, errors))
}
