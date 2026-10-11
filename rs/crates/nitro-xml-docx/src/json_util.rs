// SPDX-License-Identifier: MIT
#![allow(dead_code)]

use serde_json::{Map, Value};

pub fn defined_props(map: Map<String, Value>) -> Value {
    Value::Object(map.into_iter().filter(|(_, v)| !v.is_null()).collect())
}

pub fn omit_undefined_object(map: Map<String, Value>) -> Value {
    Value::Object(
        map.into_iter()
            .filter(|(_, v)| !matches!(v, Value::Null))
            .collect(),
    )
}

pub fn insert_if_some(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(v) = value {
        map.insert(key.to_owned(), v);
    }
}

pub fn insert_bool(map: &mut Map<String, Value>, key: &str, value: Option<bool>) {
    if let Some(v) = value {
        map.insert(key.to_owned(), Value::Bool(v));
    }
}

pub fn insert_i64(map: &mut Map<String, Value>, key: &str, value: Option<i64>) {
    if let Some(v) = value {
        map.insert(key.to_owned(), Value::Number(v.into()));
    }
}

pub fn insert_u64(map: &mut Map<String, Value>, key: &str, value: Option<u64>) {
    if let Some(v) = value {
        map.insert(key.to_owned(), Value::Number(v.into()));
    }
}

pub fn insert_str(map: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(v) = value {
        if !v.is_empty() {
            map.insert(key.to_owned(), Value::String(v));
        }
    }
}

pub fn insert_f64(map: &mut Map<String, Value>, key: &str, value: Option<f64>) {
    if let Some(v) = value {
        insert_number(map, key, v);
    }
}

pub fn insert_number(map: &mut Map<String, Value>, key: &str, value: f64) {
    let n = if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        Value::Number((value as i64).into())
    } else if let Some(n) = serde_json::Number::from_f64(value) {
        Value::Number(n)
    } else {
        return;
    };
    map.insert(key.to_owned(), n);
}
