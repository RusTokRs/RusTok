use std::str::FromStr;

use cron::Schedule;
use rhai::Dynamic;

pub fn validate_cron_expression(expression: &str) -> Result<(), String> {
    Schedule::from_str(expression)
        .map(|_| ())
        .map_err(|err| err.to_string())
}

pub fn json_to_dynamic(v: serde_json::Value) -> Dynamic {
    match v {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else if let Some(f) = n.as_f64() {
                Dynamic::from(f)
            } else {
                Dynamic::UNIT
            }
        }
        serde_json::Value::String(s) => Dynamic::from(s),
        serde_json::Value::Array(arr) => {
            let vec: Vec<Dynamic> = arr.into_iter().map(json_to_dynamic).collect();
            Dynamic::from(vec)
        }
        serde_json::Value::Object(map) => {
            let mut rhai_map = rhai::Map::new();
            for (k, v) in map {
                rhai_map.insert(k.into(), json_to_dynamic(v));
            }
            Dynamic::from(rhai_map)
        }
    }
}

pub fn dynamic_to_json(d: Dynamic) -> serde_json::Value {
    if d.is_unit() {
        serde_json::Value::Null
    } else if let Some(b) = d.clone().try_cast::<bool>() {
        serde_json::Value::Bool(b)
    } else if let Some(i) = d.clone().try_cast::<i64>() {
        serde_json::Value::Number(i.into())
    } else if let Some(f) = d.clone().try_cast::<f64>() {
        serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null)
    } else if let Some(s) = d.clone().try_cast::<String>() {
        serde_json::Value::String(s)
    } else if let Some(arr) = d.clone().try_cast::<rhai::Array>() {
        serde_json::Value::Array(arr.into_iter().map(dynamic_to_json).collect())
    } else if let Some(map) = d.clone().try_cast::<rhai::Map>() {
        let mut json_map = serde_json::Map::new();
        for (k, v) in map {
            json_map.insert(k.to_string(), dynamic_to_json(v));
        }
        serde_json::Value::Object(json_map)
    } else {
        serde_json::Value::String(d.to_string())
    }
}
