use rhai::{Dynamic, Engine, EvalAltResult, Position};
use tracing::{error, info, warn};

pub fn register_utils(engine: &mut Engine) {
    engine.register_fn("log", log_info);
    engine.register_fn("log_warn", log_warn);
    engine.register_fn("log_error", log_error);

    engine.register_fn("log", |script_name: &str, message: &str| {
        info!(target: "alloy::script", script_name = %script_name, "{}", message);
    });
    engine.register_fn("log_warn", |script_name: &str, message: &str| {
        warn!(target: "alloy::script", script_name = %script_name, "{}", message);
    });
    engine.register_fn("log_error", |script_name: &str, message: &str| {
        error!(target: "alloy::script", script_name = %script_name, "{}", message);
    });

    engine.register_fn("now", now_timestamp);
    engine.register_fn("now_unix", now_unix);

    engine.register_fn("abort", abort_script);

    engine.register_fn("format_money", format_money);
    engine.register_fn("is_empty", is_empty);
    engine.register_fn("coalesce", coalesce);
}

fn log_info(message: &str) {
    info!(target: "alloy::script", "{}", message);
}

fn log_warn(message: &str) {
    warn!(target: "alloy::script", "{}", message);
}

fn log_error(message: &str) {
    error!(target: "alloy::script", "{}", message);
}

fn now_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

fn abort_script(message: &str) -> Result<Dynamic, Box<EvalAltResult>> {
    Err(Box::new(EvalAltResult::ErrorRuntime(
        format!("ABORT:{}", message).into(),
        Position::NONE,
    )))
}

fn format_money(amount: i64) -> String {
    let s = amount.abs().to_string();
    let mut result = String::new();

    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(' ');
        }
        result.push(c);
    }

    if amount < 0 {
        result.push('-');
    }

    result.chars().rev().collect()
}

fn is_empty(value: Dynamic) -> bool {
    if value.is_unit() {
        return true;
    }
    if let Some(s) = value.clone().try_cast::<String>() {
        return s.is_empty();
    }
    if let Some(arr) = value.clone().try_cast::<Vec<Dynamic>>() {
        return arr.is_empty();
    }
    false
}

fn coalesce(value: Dynamic, default: Dynamic) -> Dynamic {
    if value.is_unit() {
        default
    } else {
        value
    }
}
