// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/utils/auto-number-format.ts and
// auto-number-scripts.ts; changes in PROVENANCE.md.

use std::collections::HashMap;

#[derive(Default)]
pub struct AutoNumberSequence {
    levels: HashMap<i32, LevelSequence>,
}

struct LevelSequence {
    scheme: String,
    ordinal: u32,
}

pub fn break_auto_number_run(sequence: &mut AutoNumberSequence, level: i32) {
    sequence.levels.retain(|k, _| *k < level);
}

pub fn next_auto_number(
    sequence: &mut AutoNumberSequence,
    level: i32,
    scheme: &str,
    start_at: u32,
) -> u32 {
    let ordinal = match sequence.levels.get(&level) {
        Some(running) if running.scheme == scheme => running.ordinal + 1,
        _ => start_at,
    };
    sequence.levels.retain(|k, _| *k <= level);
    sequence.levels.insert(
        level,
        LevelSequence {
            scheme: scheme.to_owned(),
            ordinal,
        },
    );
    ordinal
}

pub fn format_auto_number_marker(auto_num_type: &str, n: u32) -> String {
    let n = n.max(1);
    match auto_num_type {
        "arabicPeriod" => format!("{}.", n),
        "arabicDbPeriod" => format!("{}\u{FF0E}", to_full_width_digits(n)),
        "arabicParenR" => format!("{})", n),
        "arabicParenBoth" => format!("({})", n),
        "arabicPlain" => format!("{}", n),
        "arabicDbPlain" => to_full_width_digits(n),
        "alphaLcPeriod" => format!("{}.", alpha_label(n)),
        "alphaUcPeriod" => format!("{}.", alpha_label(n).to_uppercase()),
        "alphaLcParenR" => format!("{})", alpha_label(n)),
        "alphaUcParenR" => format!("{})", alpha_label(n).to_uppercase()),
        "alphaLcParenBoth" => format!("({})", alpha_label(n)),
        "alphaUcParenBoth" => format!("({})", alpha_label(n).to_uppercase()),
        "romanLcPeriod" => format!("{}.", roman_numeral(n).to_lowercase()),
        "romanUcPeriod" => format!("{}.", roman_numeral(n)),
        "romanLcParenR" => format!("{})", roman_numeral(n).to_lowercase()),
        "romanUcParenR" => format!("{})", roman_numeral(n)),
        "romanLcParenBoth" => format!("({})", roman_numeral(n).to_lowercase()),
        "romanUcParenBoth" => format!("({})", roman_numeral(n)),
        "circleNumDbPlain" => to_circled_std(n),
        "circleNumWdWhitePlain" => to_circled_std(cycle_of_ten(n)),
        "circleNumWdBlackPlain" => to_circled_black(cycle_of_ten(n)),
        other => format_script_auto_number(other, n).unwrap_or_else(|| format!("{}.", n)),
    }
}

pub fn format_auto_number_bullet(auto_num_type: &str, n: u32) -> String {
    format!("{} ", format_auto_number_marker(auto_num_type, n))
}

fn format_script_auto_number(auto_num_type: &str, n: u32) -> Option<String> {
    match auto_num_type {
        "ea1ChsPeriod" => Some(suffixed(&to_ea1_chs_numeral(n), Suffix::Period)),
        "ea1ChsPlain" => Some(to_ea1_chs_numeral(n)),
        "ea1ChtPeriod" => Some(suffixed(&to_chinese_numeral(n, true), Suffix::Period)),
        "ea1ChtPlain" => Some(to_chinese_numeral(n, true)),
        "ea1JpnChsDbPeriod" => Some(suffixed(&to_cjk_digit_string(n), Suffix::DbPeriod)),
        "ea1JpnKorPlain" => Some(to_cjk_digit_string(n)),
        "ea1JpnKorPeriod" => Some(suffixed(&to_cjk_digit_string(n), Suffix::Period)),
        "hebrew2Minus" => Some(suffixed(&hebrew_alpha_label(n), Suffix::Minus)),
        "arabic1Minus" => Some(suffixed(&repeated_label(n, ARABIC_HIJAI), Suffix::Minus)),
        "arabic2Minus" => Some(suffixed(&repeated_label(n, ARABIC_ABJADI), Suffix::Minus)),
        "hindiNumPeriod" => Some(suffixed(&to_devanagari_digits(n), Suffix::Period)),
        "hindiNumParenR" => Some(suffixed(&to_devanagari_digits(n), Suffix::ParenR)),
        "hindiAlphaPeriod" => Some(suffixed(&repeated_label(n, HINDI_VOWELS), Suffix::Period)),
        "hindiAlpha1Period" => Some(suffixed(
            &repeated_label(n, HINDI_CONSONANTS),
            Suffix::Period,
        )),
        "thaiNumPeriod" => Some(suffixed(&to_thai_digits(n), Suffix::Period)),
        "thaiNumParenR" => Some(suffixed(&to_thai_digits(n), Suffix::ParenR)),
        "thaiNumParenBoth" => Some(suffixed(&to_thai_digits(n), Suffix::ParenBoth)),
        "thaiAlphaPeriod" => Some(suffixed(
            &repeated_label(n, THAI_CONSONANTS),
            Suffix::Period,
        )),
        "thaiAlphaParenR" => Some(suffixed(
            &repeated_label(n, THAI_CONSONANTS),
            Suffix::ParenR,
        )),
        "thaiAlphaParenBoth" => Some(suffixed(
            &repeated_label(n, THAI_CONSONANTS),
            Suffix::ParenBoth,
        )),
        _ => None,
    }
}

enum Suffix {
    Period,
    DbPeriod,
    ParenR,
    ParenBoth,
    Minus,
}

fn suffixed(label: &str, suffix: Suffix) -> String {
    match suffix {
        Suffix::Period => format!("{}.", label),
        Suffix::DbPeriod => format!("{}．", label),
        Suffix::ParenR => format!("{})", label),
        Suffix::ParenBoth => format!("({})", label),
        Suffix::Minus => format!("{}-", label),
    }
}

fn roman_numeral(n: u32) -> String {
    let values = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1];
    let numerals = [
        "M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I",
    ];
    let mut remaining = n.clamp(1, 3999);
    let mut result = String::new();
    for (v, num) in values.iter().zip(numerals.iter()) {
        while remaining >= *v {
            result.push_str(num);
            remaining -= *v;
        }
    }
    result
}

fn alpha_label(n: u32) -> String {
    repeated_label(n, LATIN_LOWER)
}

fn repeated_label(n: u32, alphabet: &[char]) -> String {
    let base = alphabet.len();
    let mut remaining = n.max(1) as usize;
    let mut result = String::new();
    while remaining > 0 {
        remaining -= 1;
        result.insert(0, alphabet[remaining % base]);
        remaining /= base;
    }
    result
}

const LATIN_LOWER: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z',
];

fn cycle_of_ten(n: u32) -> u32 {
    ((n.max(1) - 1) % 10) + 1
}

fn to_circled_std(v: u32) -> String {
    if v == 0 {
        return "⓪".to_owned();
    }
    if (1..=20).contains(&v) {
        return char::from_u32(0x245f + v)
            .map(|c| c.to_string())
            .unwrap_or_else(|| format!("{}", v));
    }
    format!("{}", v)
}

fn to_circled_black(v: u32) -> String {
    if v == 0 {
        return "⓿".to_owned();
    }
    if (1..=10).contains(&v) {
        return char::from_u32(0x2775 + v)
            .map(|c| c.to_string())
            .unwrap_or_else(|| format!("{}", v));
    }
    if (11..=20).contains(&v) {
        return char::from_u32(0x24eb + (v - 11))
            .map(|c| c.to_string())
            .unwrap_or_else(|| format!("{}", v));
    }
    format!("{}", v)
}

fn map_digits(n: u32, base: u32) -> String {
    n.to_string()
        .chars()
        .map(|d| {
            let digit = d.to_digit(10).unwrap_or(0);
            char::from_u32(base + digit).unwrap_or(d)
        })
        .collect()
}

fn to_devanagari_digits(n: u32) -> String {
    map_digits(n, 0x0966)
}

fn to_thai_digits(n: u32) -> String {
    map_digits(n, 0x0e50)
}

fn to_full_width_digits(n: u32) -> String {
    map_digits(n, 0xff10)
}

fn to_cjk_digit_string(n: u32) -> String {
    const DIGITS: &[char] = &['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
    n.to_string()
        .chars()
        .map(|d| DIGITS[d.to_digit(10).unwrap_or(0) as usize])
        .collect()
}

fn to_chinese_numeral(n: u32, _traditional: bool) -> String {
    let value = n.clamp(1, 99_999_999);
    if value < 10000 {
        let label = cjk_under_10000(value);
        if (10..20).contains(&value) {
            return label.replacen('一', "", 1);
        }
        return label;
    }
    let high = value / 10000;
    let low = value % 10000;
    let mut result = cjk_under_10000(high) + "万";
    if low > 0 {
        if low < 1000 {
            result.push('零');
        }
        result.push_str(&cjk_under_10000(low));
    }
    result
}

fn cjk_under_10000(x: u32) -> String {
    const DIGITS: &[char] = &['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
    const UNITS: &[&str] = &["", "十", "百", "千"];
    let s = x.to_string();
    let len = s.len();
    let mut result = String::new();
    let mut zero_pending = false;
    let mut started = false;
    for (i, ch) in s.chars().enumerate() {
        let digit = ch.to_digit(10).unwrap_or(0) as usize;
        let unit_pos = len - 1 - i;
        if digit == 0 {
            if started {
                zero_pending = true;
            }
            continue;
        }
        if zero_pending {
            result.push(DIGITS[0]);
            zero_pending = false;
        }
        result.push(DIGITS[digit]);
        result.push_str(UNITS[unit_pos]);
        started = true;
    }
    result
}

fn to_ea1_chs_numeral(n: u32) -> String {
    let value = n.clamp(1, 99_999_999);
    if value < 100 {
        to_chinese_numeral(value, false)
    } else {
        to_cjk_digit_string(value)
    }
}

fn hebrew_alpha_label(n: u32) -> String {
    repeated_label(n, HEBREW_LETTERS)
}

const HEBREW_LETTERS: &[char] = &[
    'א', 'ב', 'ג', 'ד', 'ה', 'ו', 'ז', 'ח', 'ט', 'י', 'כ', 'ל', 'מ', 'נ', 'ס', 'ע', 'פ', 'צ', 'ק',
    'ר', 'ש', 'ת',
];

const ARABIC_HIJAI: &[char] = &[
    'أ', 'ب', 'ت', 'ث', 'ج', 'ح', 'خ', 'د', 'ذ', 'ر', 'ز', 'س', 'ش', 'ص', 'ض', 'ط', 'ظ', 'ع', 'غ',
    'ف', 'ق', 'ك', 'ل', 'م', 'ن', 'ه', 'و', 'ي',
];

const ARABIC_ABJADI: &[char] = &[
    'ا', 'ب', 'ج', 'د', 'ه', 'و', 'ز', 'ح', 'ط', 'ي', 'ك', 'ل', 'م', 'ن', 'س', 'ع', 'ف', 'ص', 'ق',
    'ر', 'ش', 'ت', 'ث', 'خ', 'ذ', 'ض', 'ظ', 'غ',
];

const HINDI_VOWELS: &[char] = &['अ', 'आ', 'इ', 'ई', 'उ', 'ऊ', 'ऋ', 'ए', 'ऐ', 'ओ', 'औ'];

const HINDI_CONSONANTS: &[char] = &[
    'क', 'ख', 'ग', 'घ', 'ङ', 'च', 'छ', 'ज', 'झ', 'ञ', 'ट', 'ठ', 'ड', 'ढ', 'ण', 'त', 'थ', 'द', 'ध',
    'न', 'प', 'फ', 'ब', 'भ', 'म', 'य', 'र', 'ल', 'व', 'श', 'ष', 'स', 'ह',
];

const THAI_CONSONANTS: &[char] = &[
    'ก', 'ข', 'ค', 'ง', 'จ', 'ฉ', 'ช', 'ซ', 'ฌ', 'ญ', 'ฎ', 'ฏ', 'ฐ', 'ฑ', 'ฒ', 'ณ', 'ด', 'ต', 'ถ',
    'ท', 'ธ', 'น', 'บ', 'ป', 'ผ', 'ฝ', 'พ', 'ฟ', 'ภ', 'ม', 'ย', 'ร', 'ล', 'ว', 'ศ', 'ษ', 'ส', 'ห',
    'ฬ', 'อ', 'ฮ',
];

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: double auto-number markers when Latin and East-Asian schemes disagree.
    #[test]
    fn ea1_chs_period_matches_upstream_table() {
        assert_eq!(format_auto_number_marker("ea1ChsPeriod", 3), "三.");
    }
}
