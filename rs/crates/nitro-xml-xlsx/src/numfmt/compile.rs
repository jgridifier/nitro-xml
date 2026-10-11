// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/compile.ts; changes listed in PROVENANCE.md.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::numfmt::fraction::{compile_fraction, fraction_slash, FractionPlan};
use crate::numfmt::number::{compile_number, NumberPlan};
use crate::numfmt::tokenizer::{split_sections, tokenize_section};
use crate::numfmt::types::{Condition, ConditionOp, SectionKind, Token};

#[derive(Debug, Clone)]
pub(crate) struct CompiledSection {
    pub tokens: Vec<Token>,
    pub kind: SectionKind,
    pub color: Option<String>,
    pub condition: Option<Condition>,
    pub number: Option<NumberPlan>,
    pub fraction: Option<FractionPlan>,
}

#[derive(Debug, Clone)]
pub(crate) struct Compiled {
    pub numeric: Vec<CompiledSection>,
    pub text: Option<CompiledSection>,
    pub conditional: bool,
    pub is_date: bool,
}

const CACHE_LIMIT: usize = 2000;

thread_local! {
    static CACHE: RefCell<HashMap<String, Compiled>> = RefCell::new(HashMap::new());
}

fn system_format(src: &str) -> String {
    let lower = src.to_ascii_lowercase();
    if lower.contains("[$-f800]") || lower.contains("[$-x-sysdate]") {
        return "dddd, mmmm d, yyyy".into();
    }
    if lower.contains("[$-f400]") || lower.contains("[$-x-systime]") {
        return "h:mm:ss AM/PM".into();
    }
    src.to_string()
}

fn compile_section(src: &str) -> CompiledSection {
    let section = tokenize_section(&system_format(src));
    let mut number = None;
    let mut fraction = None;
    if section.kind == SectionKind::Number {
        if let Some(slash) = fraction_slash(&section.tokens) {
            fraction = Some(compile_fraction(&section.tokens, slash));
        } else {
            number = Some(compile_number(&section.tokens));
        }
    }
    CompiledSection {
        tokens: section.tokens,
        kind: section.kind,
        color: section.color,
        condition: section.condition,
        number,
        fraction,
    }
}

fn general_section() -> CompiledSection {
    CompiledSection {
        tokens: vec![Token::General],
        kind: SectionKind::General,
        color: None,
        condition: None,
        number: None,
        fraction: None,
    }
}

pub(crate) fn compile_format(format: &str) -> Compiled {
    if let Some(hit) = CACHE.with(|c| c.borrow().get(format).cloned()) {
        return hit;
    }
    let code = if format.trim().is_empty() {
        "General"
    } else {
        format
    };
    let sections: Vec<CompiledSection> = split_sections(code)
        .into_iter()
        .take(4)
        .map(|s| compile_section(&s))
        .collect();
    let mut numeric;
    let mut text = None;
    let last_is_text = sections.last().is_some_and(|s| s.kind == SectionKind::Text);
    if sections.len() == 4 {
        numeric = sections[..3].to_vec();
        text = sections.get(3).cloned();
    } else if last_is_text {
        numeric = sections[..sections.len() - 1].to_vec();
        text = sections.last().cloned();
    } else {
        numeric = sections;
    }
    if numeric.is_empty() {
        numeric.push(general_section());
    }
    let conditional = numeric.iter().any(|s| s.condition.is_some());
    if conditional && numeric.len() == 1 {
        numeric.push(general_section());
    }
    let compiled = Compiled {
        numeric,
        text,
        conditional,
        is_date: false,
    };
    let compiled = Compiled {
        is_date: compiled.numeric.iter().any(|s| s.kind == SectionKind::Date),
        ..compiled
    };
    CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(format.to_string(), compiled.clone());
    });
    compiled
}

pub(crate) fn matches(c: &Condition, v: f64) -> bool {
    match c.op {
        ConditionOp::Lt => v < c.value,
        ConditionOp::Le => v <= c.value,
        ConditionOp::Gt => v > c.value,
        ConditionOp::Ge => v >= c.value,
        ConditionOp::Eq => v == c.value,
        ConditionOp::Ne => v != c.value,
    }
}

fn only_negative(c: &Condition) -> bool {
    (c.op == ConditionOp::Lt && c.value <= 0.0)
        || (c.op == ConditionOp::Le && c.value < 0.0)
        || (c.op == ConditionOp::Eq && c.value < 0.0)
}

pub(crate) struct Choice {
    pub section: CompiledSection,
    pub abs: bool,
}

pub(crate) fn choose_section(c: &Compiled, v: f64) -> Option<Choice> {
    let s1 = c.numeric.first()?;
    let s2 = c.numeric.get(1);
    let s3 = c.numeric.get(2);
    if !c.conditional {
        if v > 0.0 || s2.is_none() {
            return Some(Choice {
                section: s1.clone(),
                abs: false,
            });
        }
        if v < 0.0 {
            return Some(Choice {
                section: s2?.clone(),
                abs: true,
            });
        }
        return Some(Choice {
            section: s3.unwrap_or(s1).clone(),
            abs: false,
        });
    }
    let c1 = s1.condition;
    let c2 = s2.and_then(|s| s.condition);
    if c1.is_none() || c1.is_some_and(|cond| matches(&cond, v)) {
        return Some(Choice {
            section: s1.clone(),
            abs: c1.is_some_and(|cond| only_negative(&cond)),
        });
    }
    let s2 = s2?;
    if let Some(cond2) = c2 {
        if matches(&cond2, v) {
            return Some(Choice {
                section: s2.clone(),
                abs: only_negative(&cond2),
            });
        }
        return s3.map(|s| Choice {
            section: s.clone(),
            abs: false,
        });
    }
    if let Some(s3) = s3 {
        return Some(if v < 0.0 {
            Choice {
                section: s2.clone(),
                abs: true,
            }
        } else {
            Choice {
                section: s3.clone(),
                abs: false,
            }
        });
    }
    let c1 = c1?;
    Some(Choice {
        section: s2.clone(),
        abs: v < 0.0 && c1.op != ConditionOp::Eq && c1.value <= 0.0,
    })
}
