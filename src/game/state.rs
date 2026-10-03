use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// 游玩状态。
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GamePlayStatus {
    /// 想玩。
    Plan,
    /// 在玩。
    Playing,
    /// 玩过（通关）。
    Played,
    /// 搁置。
    OnHold,
    /// 弃坑。
    Dropped,
    /// 没有终点的游戏：长期在玩的网游、竞技类、模拟经营等。
    Endless,
}

impl GamePlayStatus {
    /// 序列化时用的字符串，与 [`TryFrom<&str>`] 的解析规则一一对应。
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Playing => "playing",
            Self::Played => "played",
            Self::OnHold => "on-hold",
            Self::Dropped => "dropped",
            Self::Endless => "endless",
        }
    }
}

impl TryFrom<&str> for GamePlayStatus {
    type Error = crate::utils::error::EnumVariantNotFound;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "plan" => Ok(Self::Plan),
            "playing" => Ok(Self::Playing),
            "played" => Ok(Self::Played),
            "on-hold" => Ok(Self::OnHold),
            "dropped" => Ok(Self::Dropped),
            "endless" => Ok(Self::Endless),
            _ => Err(crate::utils::error::EnumVariantNotFound),
        }
    }
}

/// 成就进度：已解锁 / 总数，如 `23/45`。
///
/// 成就有官方总数，所以不用百分比表示——两个数字都存着，展示和统计都方便。
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub struct GameAchievements {
    /// 已解锁数量。
    pub unlocked: u32,
    /// 成就总数。
    pub total: u32,
}

impl GameAchievements {
    #[must_use]
    pub const fn new(unlocked: u32, total: u32) -> Self {
        Self { unlocked, total }
    }

    /// 完成比例，`total` 为 0 时返回 `None`（不是 0，避免和「一个都没解锁」混淆）。
    #[must_use]
    pub fn ratio(&self) -> Option<f32> {
        if self.total == 0 {
            None
        } else {
            let unlocked = u32_to_f32(self.unlocked);
            Some(unlocked / u32_to_f32(self.total))
        }
    }
}

/// 成就计数转 `f32` 只为算展示用的比例，精度损失无所谓。
#[allow(clippy::cast_precision_loss)]
fn u32_to_f32(value: u32) -> f32 {
    value as f32
}

/// 秒转小时只为展示，精度损失无所谓。
#[allow(clippy::cast_precision_loss)]
fn u64_to_f64(value: u64) -> f64 {
    value as f64
}

impl std::fmt::Display for GameAchievements {
    /// 输出 `23/45`。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.unlocked, self.total)
    }
}

/// 用户手填的游玩状态。所有元数据刷新都不会覆盖这里。
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct GameState {
    play_status: GamePlayStatus,
    /// 游玩时长，单位：秒。
    played_seconds: Option<u64>,
    /// 成就进度，如 `23/45`。
    achievements: Option<GameAchievements>,
    /// 最后一次游玩的时间戳（Unix 秒）。Steam 提供，手填的
    /// `play_start_date` / `play_finish_date` 不受影响。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_played_at: Option<i64>,
    comment: Option<String>,
    play_start_date: Option<NaiveDate>,
    play_finish_date: Option<NaiveDate>,
}

impl GameState {
    #[must_use]
    pub const fn new(
        play_status: GamePlayStatus,
        played_seconds: Option<u64>,
        achievements: Option<GameAchievements>,
        comment: Option<String>,
        play_start_date: Option<NaiveDate>,
        play_finish_date: Option<NaiveDate>,
    ) -> Self {
        Self {
            play_status,
            played_seconds,
            achievements,
            last_played_at: None,
            comment,
            play_start_date,
            play_finish_date,
        }
    }

    /// 游玩状态。
    #[must_use]
    pub const fn play_status(&self) -> GamePlayStatus {
        self.play_status
    }

    /// 游玩时长，单位：秒。
    #[must_use]
    pub const fn played_seconds(&self) -> Option<u64> {
        self.played_seconds
    }

    /// 游玩时长（小时），仅用于展示。
    #[must_use]
    pub fn played_hours(&self) -> Option<f64> {
        self.played_seconds.map(|v| u64_to_f64(v) / 3600.0)
    }

    /// 成就进度。
    #[must_use]
    pub const fn achievements(&self) -> Option<GameAchievements> {
        self.achievements
    }

    /// 最后一次游玩的时间戳（Unix 秒）。
    #[must_use]
    pub const fn last_played_at(&self) -> Option<i64> {
        self.last_played_at
    }

    /// 最后一次游玩的日期（本地时区），仅用于展示。
    #[must_use]
    pub fn last_played_date(&self) -> Option<NaiveDate> {
        unix_seconds_to_date(self.last_played_at?)
    }

    /// 手填评论。
    #[must_use]
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    /// 开始游玩日期。
    #[must_use]
    pub const fn play_start_date(&self) -> Option<NaiveDate> {
        self.play_start_date
    }

    /// 通关日期。
    #[must_use]
    pub const fn play_finish_date(&self) -> Option<NaiveDate> {
        self.play_finish_date
    }

    pub fn playing(&mut self) {
        self.play_status = GamePlayStatus::Playing;
        self.play_start_date = Some(Utc::now().date_naive());
    }

    pub fn played(&mut self) {
        self.play_status = GamePlayStatus::Played;
        self.play_finish_date = Some(Utc::now().date_naive());
    }

    /// 标记为没有终点的游戏。
    pub fn endless(&mut self) {
        self.play_status = GamePlayStatus::Endless;
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            play_status: GamePlayStatus::Plan,
            played_seconds: None,
            achievements: None,
            last_played_at: None,
            comment: None,
            play_start_date: None,
            play_finish_date: None,
        }
    }
}

/// Steam 的分钟数 → 我们存的秒数（负数与溢出都按 0 处理）。
#[must_use]
pub fn minutes_to_seconds(minutes: i64) -> u64 {
    u64::try_from(minutes.max(0))
        .unwrap_or_default()
        .saturating_mul(60)
}

/// Unix 秒 → 本地日期。时间戳为 0 或超出 `chrono` 范围时返回 `None`。
#[must_use]
pub fn unix_seconds_to_date(seconds: i64) -> Option<NaiveDate> {
    if seconds <= 0 {
        return None;
    }
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|dt| dt.with_timezone(&chrono::Local).date_naive())
}

/// 往 `state` 里补 Steam 给的值，**只在字段为空时填**，手填的值一律保留。
///
/// 入参是 Steam 的原始单位：时长用分钟、最后游玩用 Unix 秒。
/// 返回被填上的字段名，供命令打印。
pub fn fill_from_steam(
    state: &mut GameState,
    played_minutes: i64,
    last_played_at: Option<i64>,
) -> Vec<&'static str> {
    let mut filled = Vec::new();
    if state.played_seconds.is_none() && played_minutes > 0 {
        state.played_seconds = Some(minutes_to_seconds(played_minutes));
        filled.push("played_seconds");
    }
    if state.last_played_at.is_none()
        && let Some(seconds) = last_played_at
        && seconds > 0
    {
        state.last_played_at = Some(seconds);
        filled.push("last_played_at");
    }
    filled
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [GamePlayStatus; 6] = [
        GamePlayStatus::Plan,
        GamePlayStatus::Playing,
        GamePlayStatus::Played,
        GamePlayStatus::OnHold,
        GamePlayStatus::Dropped,
        GamePlayStatus::Endless,
    ];

    #[test]
    fn as_str_agrees_with_try_from() {
        for status in ALL {
            assert_eq!(GamePlayStatus::try_from(status.as_str()).unwrap(), status);
        }
    }

    /// 导出/写库用 `as_str()`，数据文件用 serde，两者必须一致。
    #[test]
    fn as_str_agrees_with_serde() {
        for status in ALL {
            let json = serde_json::to_value(status).unwrap();
            assert_eq!(json.as_str(), Some(status.as_str()));
        }
    }

    #[test]
    fn achievements_display() {
        let a = GameAchievements::new(23, 45);
        assert_eq!(a.to_string(), "23/45");
        assert!(a.ratio().unwrap() > 0.51 && a.ratio().unwrap() < 0.52);
        assert_eq!(GameAchievements::new(0, 0).ratio(), None);
    }

    #[test]
    fn played_seconds_to_hours() {
        let state = GameState {
            played_seconds: Some(3600 * 2 + 1800),
            ..GameState::default()
        };
        assert_eq!(state.played_seconds(), Some(9000));
        assert_eq!(state.played_hours(), Some(2.5));
        assert_eq!(GameState::default().played_hours(), None);
    }
}
