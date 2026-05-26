use super::AnimeScore;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum AnimeWatchStatus {
    Plan,
    Watching,
    Watched,
    OnHold,
    Dropped,
    OriginalWorkWatched,
}

impl TryFrom<&str> for AnimeWatchStatus {
    type Error = crate::utils::error::EnumVariantNotFound;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "plan" => Ok(AnimeWatchStatus::Plan),
            "watching" => Ok(AnimeWatchStatus::Watching),
            "watched" => Ok(AnimeWatchStatus::Watched),
            "on-hold" => Ok(AnimeWatchStatus::OnHold),
            "dropped" => Ok(AnimeWatchStatus::Dropped),
            "original-work-watched" => Ok(AnimeWatchStatus::OriginalWorkWatched),
            _ => Err(crate::utils::error::EnumVariantNotFound),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnimeState {
    watch_status: AnimeWatchStatus,
    episodes_watched: u16,
    score: AnimeScore,
    comment: Option<String>,
    watch_start_date: Option<NaiveDate>,
    watch_finish_date: Option<NaiveDate>,
}

impl AnimeState {
    #[must_use]
    pub const fn new(
        watch_status: AnimeWatchStatus,
        episodes_watched: u16,
        score: AnimeScore,
        comment: Option<String>,
        watch_start_date: Option<NaiveDate>,
        watch_finish_date: Option<NaiveDate>,
    ) -> Self {
        Self {
            watch_status,
            episodes_watched,
            score,
            comment,
            watch_start_date,
            watch_finish_date,
        }
    }

    pub fn watching(&mut self) {
        self.watch_status = AnimeWatchStatus::Watching;
        self.watch_start_date = Some(Utc::now().date_naive());
    }

    pub fn watched(&mut self) {
        self.watch_status = AnimeWatchStatus::Watched;
        self.watch_finish_date = Some(Utc::now().date_naive());
    }
}

impl Default for AnimeState {
    fn default() -> Self {
        Self {
            watch_status: AnimeWatchStatus::Plan,
            episodes_watched: 0,
            score: AnimeScore::default(),
            comment: None,
            watch_start_date: None,
            watch_finish_date: None,
        }
    }
}
