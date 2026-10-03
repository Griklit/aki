use super::AnimeScore;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AnimeWatchStatus {
    Plan,
    Watching,
    Watched,
    OnHold,
    Dropped,
    OriginalWorkWatched,
}

impl AnimeWatchStatus {
    /// 序列化时用的字符串，与 [`TryFrom<&str>`] 的解析规则一一对应。
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Watching => "watching",
            Self::Watched => "watched",
            Self::OnHold => "on-hold",
            Self::Dropped => "dropped",
            Self::OriginalWorkWatched => "original-work-watched",
        }
    }
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
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

    /// 观看状态。
    #[must_use]
    pub const fn watch_status(&self) -> AnimeWatchStatus {
        self.watch_status
    }

    /// 已看集数。
    #[must_use]
    pub const fn episodes_watched(&self) -> u16 {
        self.episodes_watched
    }

    /// 手填评分。
    #[must_use]
    pub const fn score(&self) -> &AnimeScore {
        &self.score
    }

    /// 手填评论。
    #[must_use]
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    /// 开始观看日期。
    #[must_use]
    pub const fn watch_start_date(&self) -> Option<NaiveDate> {
        self.watch_start_date
    }

    /// 看完日期。
    #[must_use]
    pub const fn watch_finish_date(&self) -> Option<NaiveDate> {
        self.watch_finish_date
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

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [AnimeWatchStatus; 6] = [
        AnimeWatchStatus::Plan,
        AnimeWatchStatus::Watching,
        AnimeWatchStatus::Watched,
        AnimeWatchStatus::OnHold,
        AnimeWatchStatus::Dropped,
        AnimeWatchStatus::OriginalWorkWatched,
    ];

    #[test]
    fn as_str_agrees_with_try_from() {
        for status in ALL {
            assert_eq!(AnimeWatchStatus::try_from(status.as_str()).unwrap(), status);
        }
    }

    /// `export-state` 用 `as_str()` 写库，数据文件用 serde。两者必须一致，
    /// 否则同一个状态在 JSON 和数据库里会显示成不同的值。
    #[test]
    fn as_str_agrees_with_serde() {
        for status in ALL {
            let json = serde_json::to_value(status).unwrap();
            assert_eq!(json.as_str(), Some(status.as_str()));
        }
    }
}
