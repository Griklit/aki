use std::fmt;
use std::str::FromStr;

use chrono::{Datelike, Local};

/// 一年里各季度的开播月份：1 月、4 月、7 月、10 月。
const QUARTER_MONTHS: [u32; 4] = [1, 4, 7, 10];

/// 番剧的一个季度：1 月 / 4 月 / 7 月 / 10 月开播的那三个月。
///
/// 例如「2026 年 1 月番」= 2026-01-01 ~ 2026-03-31，用接口的 `air_date` 条件表达就是
/// `>=2026-01-01` 且 `<2026-04-01`（上界取开区间，正好把 3.31 包含进来）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Season {
    year: i32,
    /// 1~4，分别对应 1 / 4 / 7 / 10 月开播。
    quarter: u8,
}

#[derive(Debug, thiserror::Error)]
pub enum SeasonParseError {
    #[error(
        "unrecognized season '{0}' (expected current / last / next / 2026-10 / 2026-Q4, \
         the month must be 1, 4, 7 or 10)"
    )]
    Invalid(String),
}

impl Season {
    /// `quarter` 取 1~4（1 月 / 4 月 / 7 月 / 10 月开播），年份限制在 1900~2100。
    #[must_use]
    pub fn new(year: i32, quarter: u8) -> Option<Self> {
        ((1..=4).contains(&quarter) && (1900..=2100).contains(&year))
            .then_some(Self { year, quarter })
    }

    /// 某个日期落在哪个季度（1~3 月是第 1 季，依此类推）。
    #[must_use]
    pub fn of(date: chrono::NaiveDate) -> Self {
        let quarter = u8::try_from((date.month() - 1) / 3 + 1).unwrap_or(1);
        Self {
            year: date.year(),
            quarter,
        }
    }

    /// 今天所在的季度。
    #[must_use]
    pub fn current() -> Self {
        Self::of(Local::now().date_naive())
    }

    #[must_use]
    pub fn year(self) -> i32 {
        self.year
    }

    #[must_use]
    pub fn quarter(self) -> u8 {
        self.quarter
    }

    /// 开播月份：1 / 4 / 7 / 10。
    #[must_use]
    pub fn month(self) -> u32 {
        u32::from(self.quarter) * 3 - 2
    }

    /// 下一个季度（10 月番的下一个是明年 1 月番）。
    #[must_use]
    pub fn next(self) -> Self {
        if self.quarter == 4 {
            Self {
                year: self.year + 1,
                quarter: 1,
            }
        } else {
            Self {
                year: self.year,
                quarter: self.quarter + 1,
            }
        }
    }

    /// 上一个季度（1 月番的上一个是去年 10 月番）。
    #[must_use]
    pub fn prev(self) -> Self {
        if self.quarter == 1 {
            Self {
                year: self.year - 1,
                quarter: 4,
            }
        } else {
            Self {
                year: self.year,
                quarter: self.quarter - 1,
            }
        }
    }

    /// 接口 `air_date` 用的两个条件：`[">=本季首日", "<下季首日"]`。
    ///
    /// 上界是开区间，所以 1 月番正好覆盖到 3.31。
    #[must_use]
    pub fn air_date_filter(self) -> [String; 2] {
        let next = self.next();
        [
            format!(">={:04}-{:02}-01", self.year, self.month()),
            format!("<{:04}-{:02}-01", next.year, next.month()),
        ]
    }
}

impl FromStr for Season {
    type Err = SeasonParseError;

    /// 认这几种写法（大小写不敏感）：
    ///
    /// | 写法 | 含义 |
    /// |------|------|
    /// | `current` / `this` / `now` | 本季 |
    /// | `last` / `prev` | 上一季 |
    /// | `next` | 下一季 |
    /// | `2026-10` | 2026 年 10 月番（月份只能是 1 / 4 / 7 / 10） |
    /// | `2026-Q4` | 同上，用季度序号写 |
    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        let spec = spec.trim();
        let invalid = || SeasonParseError::Invalid(spec.to_string());
        match spec.to_ascii_lowercase().as_str() {
            "current" | "this" | "now" => return Ok(Self::current()),
            "last" | "prev" | "previous" => return Ok(Self::current().prev()),
            "next" => return Ok(Self::current().next()),
            _ => {}
        }

        let (year, rest) = spec.split_once('-').ok_or_else(invalid)?;
        let year: i32 = year.parse().map_err(|_| invalid())?;
        // `Q4` / `q4` 是季度序号，`10` / `4` 是开播月份，两者都能定位同一个季度。
        let quarter = if let Some(quarter) = rest
            .strip_prefix(['q', 'Q'])
            .and_then(|quarter| quarter.parse::<u8>().ok())
        {
            quarter
        } else {
            let month: u32 = rest.parse().map_err(|_| invalid())?;
            QUARTER_MONTHS
                .iter()
                .position(|known| *known == month)
                .and_then(|index| u8::try_from(index + 1).ok())
                .ok_or_else(invalid)?
        };
        Self::new(year, quarter).ok_or_else(invalid)
    }
}

impl fmt::Display for Season {
    /// 写成接口/CLI 也认的 `2026-10` 形式。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month())
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::Season;

    fn season(spec: &str) -> Season {
        spec.parse().unwrap()
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    /// 月份落在哪一季：1~3 月是 1 月番，10~12 月是 10 月番。
    #[test]
    fn dates_map_to_quarters() {
        assert_eq!(Season::of(date(2026, 1, 1)), season("2026-01"));
        assert_eq!(Season::of(date(2026, 3, 31)), season("2026-01"));
        assert_eq!(Season::of(date(2026, 4, 1)), season("2026-04"));
        assert_eq!(Season::of(date(2026, 10, 1)), season("2026-10"));
        assert_eq!(Season::of(date(2026, 12, 31)), season("2026-Q4"));
    }

    /// 1 月番 = 1.1~3.31：上界用下季首日的开区间表示。
    #[test]
    fn air_date_filter_covers_the_whole_quarter() {
        assert_eq!(
            season("2026-01").air_date_filter(),
            [">=2026-01-01".to_string(), "<2026-04-01".to_string()]
        );
        // 10 月番要跨年
        assert_eq!(
            season("2026-Q4").air_date_filter(),
            [">=2026-10-01".to_string(), "<2027-01-01".to_string()]
        );
    }

    #[test]
    fn parse_accepts_documented_forms() {
        assert_eq!(season("2026-10"), Season::new(2026, 4).unwrap());
        assert_eq!(season("2026-Q4"), Season::new(2026, 4).unwrap());
        assert_eq!(season("2026-q1"), Season::new(2026, 1).unwrap());
        assert_eq!(season(" 2026-04 "), Season::new(2026, 2).unwrap());
        assert_eq!(season("2026-4"), Season::new(2026, 2).unwrap(), "4 是 4 月");
        assert_eq!(season("2026-1"), Season::new(2026, 1).unwrap());
    }

    #[test]
    fn parse_rejects_other_months_and_junk() {
        // 5 月不是任何一季的开播月
        assert!("2026-05".parse::<Season>().is_err());
        assert!("2026-12".parse::<Season>().is_err());
        assert!("2026-0".parse::<Season>().is_err());
        assert!("2026".parse::<Season>().is_err());
        assert!("2026-Q5".parse::<Season>().is_err());
        assert!("2026-13".parse::<Season>().is_err());
        assert!("abc".parse::<Season>().is_err());
        // 年份越界
        assert!("1800-01".parse::<Season>().is_err());
    }

    /// `Display` 写出来的就是 `FromStr` 认的写法。
    #[test]
    fn display_round_trips() {
        for spec in ["2026-01", "2026-04", "2026-Q3", "2026-10"] {
            let parsed = season(spec);
            assert_eq!(parsed.to_string().parse::<Season>().unwrap(), parsed);
        }
        assert_eq!(season("2026-Q4").to_string(), "2026-10");
    }

    #[test]
    fn next_and_prev_wrap_across_the_year() {
        assert_eq!(season("2026-10").next(), season("2027-01"));
        assert_eq!(season("2026-01").prev(), season("2025-10"));
        assert_eq!(season("2026-04").next(), season("2026-07"));
        assert_eq!(season("2026-07").prev(), season("2026-04"));
    }
}
