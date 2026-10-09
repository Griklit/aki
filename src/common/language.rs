use core::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    ZhCn,
    JaJp,
    EnUs,
}

impl Language {
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::JaJp => "ja-JP",
            Self::EnUs => "en-US",
        }
    }

    #[must_use]
    pub fn language(&self) -> &'static str {
        match self {
            Self::ZhCn => "zh",
            Self::JaJp => "ja",
            Self::EnUs => "en",
        }
    }

    #[must_use]
    pub fn region(&self) -> &'static str {
        match self {
            Self::ZhCn => "CN",
            Self::JaJp => "JP",
            Self::EnUs => "US",
        }
    }

    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::ZhCn, Self::JaJp, Self::EnUs]
    }

    /// 检测文本所属语言
    #[must_use]
    pub fn detect_from(text: &str) -> Self {
        let mut kana = 0;
        // let mut english_alphabet = 0;
        let mut chinese_character = 0;
        let mut all = 0;
        for c in text.chars() {
            let v = c as u32;
            match v {
                // English alphabet
                // 0x41..=0x5A | 0x61..=0x7A => english_alphabet += 1,
                // Hiragana and Katakana
                0x3040..=0x30FF => kana += 1,
                // Chinese characters
                0x4E00..=0x9FFF => chinese_character += 1,
                _ => {}
            }
            all += 1;
        }
        if f64::from(kana) / f64::from(all) > 0.05 {
            return Self::JaJp;
        }
        if f64::from(chinese_character) / f64::from(all) > 0.1 {
            return Self::ZhCn;
        }
        Self::EnUs
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind())
    }
}

impl FromStr for Language {
    type Err = crate::utils::error::EnumVariantNotFound;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "zh-CN" | "zh-cn" => Ok(Self::ZhCn),
            "ja-JP" | "ja-jp" => Ok(Self::JaJp),
            "en-US" | "en-us" => Ok(Self::EnUs),
            _ => Err(crate::utils::error::EnumVariantNotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn test_detect_language() {
        let zh_cn = "物语系列 外传季&怪物季";
        let ja_jp = "物語〉シリーズ オフ&モンスターシーズン";
        let en_us = "Monogatari Series: Off & Monster Season";
        assert_eq!(Language::detect_from(zh_cn), Language::ZhCn);
        assert_eq!(Language::detect_from(ja_jp), Language::JaJp);
        assert_eq!(Language::detect_from(en_us), Language::EnUs);
    }
}
