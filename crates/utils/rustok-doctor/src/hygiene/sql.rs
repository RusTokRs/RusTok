use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;

macro_rules! re {
    ($pattern:expr) => {{
        static REGEX: std::sync::LazyLock<Regex> =
            std::sync::LazyLock::new(|| Regex::new($pattern).expect("valid SQL rule"));
        &*REGEX
    }};
}

fn close(open: u8) -> Option<u8> {
    match open {
        b'(' => Some(b')'),
        b'[' => Some(b']'),
        b'{' => Some(b'}'),
        _ => None,
    }
}

fn literal(value: &str) -> bool {
    let value = value.trim();
    if matches!(value, "true" | "false")
        || re!(r"^[+-]?\d[\d_]*(?:[iu](?:\d+|size))?$").is_match(value)
    {
        return true;
    }
    if value.starts_with('"') && value.ends_with('"') {
        return true;
    }
    if let Some(rest) = value.strip_prefix('r') {
        let count = rest.bytes().take_while(|byte| *byte == b'#').count();
        let delimiter = format!("\"{}", "#".repeat(count));
        return rest[count..].starts_with('"') && rest.ends_with(&delimiter);
    }
    false
}

fn dynamic_format(code: &str, syntax: &str, opening: usize) -> bool {
    let mut closings = match syntax.as_bytes().get(opening).and_then(|byte| close(*byte)) {
        Some(value) => vec![value],
        None => return false,
    };
    let mut arguments = Vec::new();
    let mut start = opening + 1;
    for (offset, delimiter) in syntax.bytes().enumerate().skip(start) {
        if let Some(end) = close(delimiter) {
            closings.push(end);
        } else if closings.last() == Some(&delimiter) {
            closings.pop();
            if closings.is_empty() {
                arguments.push(code[start..offset].trim());
                break;
            }
        } else if delimiter == b',' && closings.len() == 1 {
            arguments.push(code[start..offset].trim());
            start = offset + 1;
        }
    }
    if !closings.is_empty() || arguments.is_empty() {
        return false;
    }
    let mut explicit_names = BTreeSet::new();
    for argument in arguments.iter().skip(1) {
        let mut value = *argument;
        if value.is_empty() {
            continue;
        }
        if let Some(capture) = re!(r"^\s*([A-Za-z_]\w*)\s*=\s*").captures(value) {
            explicit_names.insert(capture[1].to_owned());
            value = &value[capture.get(0).expect("assignment exists").end()..];
        }
        if !literal(value) {
            return true;
        }
    }
    let template = arguments[0].replace("{{", "").replace("}}", "");
    re!(r"\{([A-Za-z_]\w*)[:}]")
        .captures_iter(&template)
        .any(|capture| !explicit_names.contains(&capture[1]))
}

pub(super) fn check(
    code: &str,
    syntax: &str,
    depth: usize,
    bindings: &mut BTreeMap<usize, BTreeMap<String, bool>>,
) -> bool {
    let assignment = re!(r"^\s*let\s+(?:mut\s+)?([A-Za-z_]\w*)(?:\s*:\s*[^=;]+)?\s*=\s*")
        .captures(syntax)
        .or_else(|| re!(r"^\s*([A-Za-z_]\w*)\s*=\s*").captures(syntax));
    let format_call = re!(r"(?:std::)?format\s*!\s*[({\[]");
    if let Some(assignment) = assignment {
        let name = &assignment[1];
        let after = assignment.get(0).expect("assignment exists").end();
        let format = format_call
            .find_at(syntax, after)
            .filter(|found| found.start() == after);
        if format.is_some() || bindings.values().any(|scope| scope.contains_key(name)) {
            let dynamic = format.is_some_and(|found| dynamic_format(code, syntax, found.end() - 1));
            bindings
                .entry(depth)
                .or_default()
                .insert(name.to_owned(), dynamic);
        }
    }
    let first_line_end = syntax.find('\n').unwrap_or(syntax.len());
    for call in re!(r"\.\s*execute_unprepared\s*\(\s*&?\s*").find_iter(syntax) {
        if call.start() > first_line_end {
            break;
        }
        if let Some(found) = format_call
            .find_at(syntax, call.end())
            .filter(|found| found.start() == call.end())
        {
            if dynamic_format(code, syntax, found.end() - 1) {
                return true;
            }
        }
        if let Some(argument) = re!(r"^([A-Za-z_]\w*)\s*,?\s*\)").captures(&syntax[call.end()..]) {
            for scope in bindings.values().rev() {
                if let Some(dynamic) = scope.get(&argument[1]) {
                    return *dynamic;
                }
            }
        }
    }
    false
}
