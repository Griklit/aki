use chrono::NaiveDate;
use std::sync::LazyLock;

/// 月名（英文简写与全称、大小写不敏感）→ 月份。
pub static MONTH_NAMES: LazyLock<Vec<(String, u8)>> = LazyLock::new(|| {
    const FULL: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    let mut names: Vec<(String, u8)> = Vec::with_capacity(24);
    for (i, name) in FULL.iter().enumerate() {
        let month = u8::try_from(i + 1).expect("month index fits in u8");
        names.push(((*name).to_string(), month));
        names.push((name[..3].to_string(), month));
    }
    names
});

/// 字符串里出现的年份：任意一个 4 位数（[1900, 2100] 之内）。
#[must_use]
pub fn detect_year(raw: &str) -> Option<i32> {
    raw.split(|c: char| !c.is_ascii_digit())
        .filter(|s| s.len() == 4)
        .filter_map(|s| s.parse::<i32>().ok())
        .find(|year| (1900..=2100).contains(year))
}

/// 找出字符串里的月名。
#[must_use]
pub fn detect_month(raw: &str) -> Option<u8> {
    let lower = raw.to_lowercase();
    MONTH_NAMES
        .iter()
        .find(|(name, _)| lower.contains(name.as_str()))
        .map(|(_, month)| *month)
}

/// 从 `raw[..end]` 的末尾取出连续的数字（先跳过空白）。
fn trailing_digits(raw: &str, end: usize) -> String {
    let mut digits: Vec<char> = Vec::new();
    let mut seen_digit = false;
    for c in raw[..end].chars().rev() {
        if c.is_ascii_digit() {
            seen_digit = true;
            digits.push(c);
        } else if seen_digit || !c.is_whitespace() {
            break;
        }
    }
    digits.reverse();
    digits.into_iter().collect()
}

/// 中文写法 `7 月` 里的月份。
#[must_use]
pub fn detect_numeric_month(raw: &str) -> Option<u8> {
    for (i, _) in raw.match_indices('月') {
        if let Ok(month) = trailing_digits(raw, i).parse::<u8>()
            && (1..=12).contains(&month)
        {
            return Some(month);
        }
    }
    None
}

/// 季度：`Q1`~`Q4`，或中文的 `第 1 季度`。
#[must_use]
pub fn detect_quarter(raw: &str) -> Option<u8> {
    let lower = raw.to_lowercase();
    for (i, pattern) in ["q1", "q2", "q3", "q4"].iter().enumerate() {
        if lower.contains(pattern) {
            return u8::try_from(i + 1).ok();
        }
    }
    for (i, _) in raw.match_indices('季') {
        if let Ok(quarter) = trailing_digits(raw, i).parse::<u8>()
            && (1..=4).contains(&quarter)
        {
            return Some(quarter);
        }
    }
    None
}

/// 日号：排除年份和月份之后，第一个落在 [1, 31] 的数字。
fn detect_day(raw: &str, year: Option<i32>, month: Option<u8>) -> Option<u32> {
    let year = year.and_then(|y| u32::try_from(y).ok());
    raw.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u32>().ok())
        .find(|v| (1..=31).contains(v) && Some(*v) != year && Some(*v) != month.map(u32::from))
}

/// 从脏字符串里尽量解析出「年-月-日」，粒度不足时返回 `None`。
#[must_use]
pub fn parse_date(raw: &str) -> Option<NaiveDate> {
    let year = detect_year(raw)?;
    let month = detect_numeric_month(raw).or_else(|| detect_month(raw))?;
    let day = detect_day(raw, Some(year), Some(month))?;
    NaiveDate::from_ymd_opt(year, u32::from(month), day)
}

/// 解析 Steam 的日期字符串，精确到日才返回 `Some`。
#[must_use]
pub fn parse_steam_date(raw: &str) -> Option<NaiveDate> {
    parse_date(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_english_date() {
        let day = NaiveDate::from_ymd_opt(2013, 7, 9).unwrap();
        assert_eq!(parse_steam_date("9 Jul, 2013"), Some(day));
        assert_eq!(parse_steam_date("Jul 9, 2013"), Some(day));
        assert_eq!(parse_steam_date("July 9, 2013"), Some(day));
        assert_eq!(
            parse_steam_date("22 August 2024"),
            NaiveDate::from_ymd_opt(2024, 8, 22)
        );
    }

    #[test]
    fn parse_chinese_date() {
        assert_eq!(
            parse_steam_date("2013 年 7 月 9 日"),
            NaiveDate::from_ymd_opt(2013, 7, 9)
        );
        assert_eq!(
            parse_steam_date("2024 年 12 月 31 日"),
            NaiveDate::from_ymd_opt(2024, 12, 31)
        );
    }

    #[test]
    fn refuse_partial_date() {
        assert_eq!(parse_steam_date("2024"), None);
        assert_eq!(parse_steam_date("Jan 2024"), None);
        assert_eq!(parse_steam_date("2025 年 第 1 季度"), None);
        assert_eq!(parse_steam_date("即将推出"), None);
        assert_eq!(parse_steam_date("To be announced"), None);
        assert_eq!(parse_steam_date(""), None);
    }

    #[test]
    fn detect_year_month_quarter() {
        assert_eq!(detect_year("2025 年 第 1 季度"), Some(2025));
        assert_eq!(detect_year("9 Jul, 2013"), Some(2013));
        assert_eq!(detect_year("即将推出"), None);
        assert_eq!(detect_quarter("2025 年 第 1 季度"), Some(1));
        assert_eq!(detect_quarter("Q4 2025"), Some(4));
        assert_eq!(detect_quarter("Second Quarter 2025"), None);
        assert_eq!(detect_month("Jan 2024"), Some(1));
        assert_eq!(detect_month("2013 年 7 月 9 日"), None);
        assert_eq!(detect_numeric_month("2013 年 7 月 9 日"), Some(7));
    }
}
