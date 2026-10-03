use crate::{CONFIG, Namespace};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub struct GameNamespace;

impl Namespace for GameNamespace {
    fn new() -> Self {
        GameNamespace
    }

    fn kind() -> &'static str {
        "game"
    }

    fn data_path() -> PathBuf {
        CONFIG.game_data_path().to_path_buf()
    }
}
