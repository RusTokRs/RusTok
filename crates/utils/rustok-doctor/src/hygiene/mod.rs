mod sql;

use std::collections::BTreeMap;
use std::path::Path;

use regex::Regex;
use serde::Serialize;

use crate::source;

macro_rules! re {
    ($pattern:expr) => {{
        static REGEX: std::sync::LazyLock<Regex> =
            std::sync::LazyLock::new(|| Regex::new($pattern).expect("valid hygiene rule"));
        &*REGEX
    }};
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Severity {
    #[serde(rename = "P0-CRITICAL")]
    Critical,
    #[serde(rename = "P1-HIGH")]
    High,
    #[serde(rename = "P2-MEDIUM")]
    Medium,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Heuristic,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub pattern_id: &'static str,
    pub rule_name: &'static str,
    pub severity: Severity,
    pub confidence: Confidence,
    pub file: String,
    pub line: usize,
    pub content: String,
    pub message: String,
    pub suggested_fix: Option<String>,
}

struct Scan<'a> {
    file: String,
    lines: Vec<source::SourceLine<'a>>,
    findings: Vec<Finding>,
    test_file: bool,
    build_file: bool,
    migration: bool,
    vendored: bool,
    services: bool,
    error_file: bool,
}

impl Scan<'_> {
    fn add(
        &mut self,
        line: usize,
        pattern_id: &'static str,
        rule_name: &'static str,
        severity: Severity,
        message: impl Into<String>,
        fix: impl Into<String>,
    ) {
        self.findings.push(Finding {
            pattern_id,
            rule_name,
            severity,
            confidence: if pattern_id == "P-ZERO-INCLUDE" {
                Confidence::High
            } else {
                Confidence::Heuristic
            },
            file: self.file.clone(),
            line,
            content: self.lines[line - 1].code.trim().to_owned(),
            message: message.into(),
            suggested_fix: Some(fix.into()),
        });
    }
}

fn closing(open: u8) -> Option<u8> {
    match open {
        b'(' => Some(b')'),
        b'[' => Some(b']'),
        b'{' => Some(b'}'),
        _ => None,
    }
}

fn discarded_result_propagates(statement: &str) -> bool {
    let mut delimiters = Vec::new();
    for (offset, byte) in statement.bytes().enumerate() {
        if let Some(end) = closing(byte) {
            delimiters.push(end);
        } else if delimiters.last() == Some(&byte) {
            delimiters.pop();
        } else if byte == b';' && delimiters.is_empty() {
            return statement[..offset].trim_end().ends_with('?');
        }
    }
    false
}

fn json_decode_defaults(syntax: &str, first_line_len: usize) -> bool {
    let decode = re!(r"\bserde_json\s*::\s*from_(?:value|str|slice)\s*(?:::<[\w\s:,<>]+>)?\s*\(");
    for found in decode
        .find_iter(syntax)
        .take_while(|found| found.start() < first_line_len)
    {
        let mut delimiters = vec![b')'];
        for (relative, byte) in syntax.as_bytes()[found.end()..].iter().copied().enumerate() {
            if let Some(end) = closing(byte) {
                delimiters.push(end);
            } else if delimiters.last() == Some(&byte) {
                delimiters.pop();
                if delimiters.is_empty() {
                    let rest = &syntax[found.end() + relative + 1..];
                    if re!(r"^\s*\.\s*unwrap_or_default\s*\(\s*\)").is_match(rest) {
                        return true;
                    }
                    break;
                }
            }
        }
    }
    false
}

fn filter_discards_errors(syntax: &str, first_line_len: usize) -> bool {
    let filter = re!(r"\.\s*filter_map\s*\(");
    let error_to_none = re!(r"\.\s*ok\s*\(\s*\)|\bResult\s*::\s*ok\b");
    for found in filter
        .find_iter(syntax)
        .take_while(|found| found.start() < first_line_len)
    {
        let mut delimiters = vec![b')'];
        for (relative, byte) in syntax.as_bytes()[found.end()..].iter().copied().enumerate() {
            if let Some(end) = closing(byte) {
                delimiters.push(end);
            } else if delimiters.last() == Some(&byte) {
                delimiters.pop();
                if delimiters.is_empty() {
                    if error_to_none.is_match(&syntax[found.end()..found.end() + relative]) {
                        return true;
                    }
                    break;
                }
            }
        }
    }
    false
}

fn orphan_task_spawn(syntax: &str, first_line_len: usize) -> bool {
    let spawn = re!(
        r"^\s*(?:let\s+_\s*=\s*)?(?:(?:tokio|std\s*::\s*thread)\s*::\s*(?:task\s*::\s*)?)?spawn\s*\("
    );
    for found in spawn
        .find_iter(syntax)
        .take_while(|found| found.start() < first_line_len)
    {
        let mut delimiters = vec![b')'];
        let after_open = found.end();
        for (relative, byte) in syntax.as_bytes()[after_open..].iter().copied().enumerate() {
            if let Some(end) = closing(byte) {
                delimiters.push(end);
            } else if delimiters.last() == Some(&byte) {
                delimiters.pop();
                if delimiters.is_empty() {
                    let rest = syntax[after_open + relative + 1..].trim_start();
                    if rest.starts_with(';') {
                        return true;
                    }
                    break;
                }
            }
        }
    }
    false
}

fn unbounded_network_allocation(window: &[&str]) -> Option<String> {
    let parse_len = re!(r"\blet\s+(?:mut\s+)?([a-zA-Z_]\w*)\s*=\s*[^;\n]*from_(?:be|le)_bytes\b");
    let line = window.first()?;
    let captures = parse_len.captures(line)?;
    let var_name = captures[1].to_owned();

    for following in window.iter().skip(1).take(8) {
        if following.contains("MAX_")
            || following.contains("ensure_")
            || following.contains("validate_")
            || following.contains(".min(")
            || following.contains(".max(")
            || following.contains(&format!("{var_name} >"))
            || following.contains(&format!("{var_name} <"))
            || following.contains(&format!("{var_name} =="))
            || following.contains(&format!("contains(&{var_name})"))
            || following.contains(&format!("contains(&&{var_name})"))
        {
            return None;
        }
        if re!(r"\b(?:vec!\s*\[|Vec\s*::\s*with_capacity\s*\()").is_match(following)
            && following.contains(&var_name)
        {
            return Some(var_name);
        }
    }
    None
}

fn reactive_boundaries(syntax: &str) -> Vec<usize> {
    let tokens = re!(
        r"(?P<reactive>\b(?:create_effect|create_memo|create_resource|create_local_resource|Effect::new|Memo::new|Resource::new|LocalResource::new)\s*\(|\bview!\s*[{(\[])|[(){}\[\]]"
    );
    let mut closings = Vec::new();
    let mut reactive_depth = None;
    let mut boundaries = Vec::new();
    for captures in tokens.captures_iter(syntax) {
        let token = captures.get(0).expect("token exists");
        let delimiter = token.as_str().as_bytes()[token.len() - 1];
        if let Some(end) = closing(delimiter) {
            closings.push(end);
            if captures.name("reactive").is_some() && reactive_depth.is_none() {
                reactive_depth = Some(closings.len());
                boundaries.push(token.end());
            }
        } else if closings.last() == Some(&delimiter) {
            if reactive_depth == Some(closings.len()) {
                boundaries.push(token.start());
                reactive_depth = None;
            }
            closings.pop();
        }
    }
    boundaries
}

#[derive(Default)]
struct TransactionScope {
    depth: usize,
    transaction: bool,
    publications: Vec<usize>,
}

fn transactional_publications(syntax: &str) -> Vec<usize> {
    if !syntax.contains("publish") {
        return Vec::new();
    }
    let tokens = re!(
        r"(?P<function>\bfn\s+(?:r#)?[A-Za-z_]\w*)|(?P<transaction>\bDatabaseTransaction\b|\.\s*begin(?:_with_config)?\s*\()|(?P<publish>\b(?:event_bus|event_publisher)\s*\.\s*publish\s*\()|[(){}\[\];]"
    );
    let mut pending: Option<TransactionScope> = None;
    let mut functions: Vec<TransactionScope> = Vec::new();
    let mut closings = Vec::new();
    let mut publications = Vec::new();
    for captures in tokens.captures_iter(syntax) {
        let token = captures.get(0).expect("token exists");
        let delimiter = token.as_str().as_bytes()[token.len() - 1];
        if captures.name("function").is_some() {
            pending = Some(TransactionScope {
                depth: closings.len(),
                ..Default::default()
            });
            continue;
        }
        if captures.name("transaction").is_some() {
            if let Some(scope) = pending.as_mut().or_else(|| functions.last_mut()) {
                scope.transaction = true;
            }
        }
        if captures.name("publish").is_some() && pending.is_none() {
            if let Some(scope) = functions.last_mut() {
                scope.publications.push(token.start());
            }
        }
        if let Some(end) = closing(delimiter) {
            if delimiter == b'{'
                && pending
                    .as_ref()
                    .is_some_and(|scope| scope.depth == closings.len())
            {
                functions.push(pending.take().expect("pending scope exists"));
            }
            closings.push(end);
        } else if closings.last() == Some(&delimiter) {
            closings.pop();
            if functions
                .last()
                .is_some_and(|scope| scope.depth == closings.len())
            {
                let scope = functions.pop().expect("function exists");
                if scope.transaction {
                    publications.extend(scope.publications);
                }
            }
        } else if delimiter == b';'
            && pending
                .as_ref()
                .is_some_and(|scope| scope.depth == closings.len())
        {
            pending = None;
        }
    }
    publications.sort_unstable();
    publications
}

fn matched_with_literals<'a>(
    regex: &Regex,
    code: &'a str,
    syntax: &str,
) -> Option<regex::Captures<'a>> {
    regex.captures_iter(code).find(|capture| {
        capture.get(0).is_some_and(|matched| {
            syntax
                .as_bytes()
                .get(matched.start())
                .is_some_and(|byte| !byte.is_ascii_whitespace())
        })
    })
}

/// Scan one Rust source with a caller-supplied path, which controls context exemptions.
pub fn scan_source(path: &Path, content: &str) -> Vec<Finding> {
    let file = path.to_string_lossy().replace('\\', "/");
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    let parts: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let test_file = parts.iter().any(|part| {
        matches!(
            part.as_str(),
            "tests"
                | "benches"
                | "rustok-test-utils"
                | "test_helpers"
                | "test_support"
                | "testing"
                | "fixtures"
                | "mocks"
        )
    }) || [
        "_test.rs",
        "_tests.rs",
        "_test_support.rs",
        "mock.rs",
        "stub.rs",
        "_evidence.rs",
    ]
    .iter()
    .any(|suffix| name.ends_with(suffix))
        || matches!(name.as_str(), "test.rs" | "tests.rs" | "test_support.rs");
    let mut scan = Scan {
        file,
        lines: source::lines(content),
        findings: Vec::new(),
        test_file,
        build_file: name == "build.rs" || parts.iter().any(|part| part == "examples"),
        migration: name.starts_with("m202") || parts.iter().any(|part| part.contains("migration")),
        vendored: parts.iter().any(|part| part.contains("vendored")),
        services: parts
            .iter()
            .any(|part| part == "services" || part == "service")
            || matches!(name.as_str(), "service.rs" | "services.rs")
            || name.ends_with("_service.rs"),
        error_file: name.contains("error"),
    };
    let syntax = scan
        .lines
        .iter()
        .map(|line| line.syntax.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let reactive = reactive_boundaries(&syntax);
    let publications = transactional_publications(&syntax);
    let has_unprepared_sql = syntax.contains("execute_unprepared");
    let trigger_candidates = if scan.migration {
        let code = scan
            .lines
            .iter()
            .map(|line| line.code.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        sql::column_update_triggers(&code, &syntax)
    } else {
        Vec::new()
    };
    let mut offset = 0;
    let mut depth = 0usize;
    let mut test_scopes = Vec::new();
    let mut pending_test = false;
    let mut loop_depths = Vec::new();
    let mut pending_loop = false;
    let mut static_depth = None;
    let mut pending_static = false;
    let mut static_delimiters = Vec::new();
    let mut async_depth = None;
    let mut pending_async = false;
    let mut in_fn_params = false;
    let mut fn_param_parens = 0usize;
    let mut guards: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut sql_bindings = BTreeMap::new();

    for index in 0..scan.lines.len() {
        let line_number = index + 1;
        let syntax_line = scan.lines[index].syntax.clone();
        let comment = scan.lines[index].comment.clone();
        let code_part = syntax_line.trim();
        let line_start = offset;
        let statement_offset = line_start + syntax_line.len() - syntax_line.trim_start().len();
        offset += syntax_line.len() + 1;

        let test_attribute =
            re!(r"#\s*\[\s*(?:cfg\s*\(\s*test\s*\)|(?:tokio::)?test(?:\s*\([^]]*\))?)\s*\]")
                .is_match(code_part)
                || re!(r"\bmod\s+tests?\b").is_match(code_part);
        if !scan.test_file
            && !scan.build_file
            && !scan.migration
            && test_scopes.is_empty()
            && !pending_test
            && !test_attribute
            && re!(r"^#\s*\[\s*allow\s*\(\s*dead_code\s*\)\s*\]").is_match(code_part)
        {
            let next_code = scan
                .lines
                .iter()
                .skip(index + 1)
                .take(3)
                .map(|l| l.syntax.trim())
                .find(|s| !s.is_empty())
                .unwrap_or("");
            if re!(r"\b(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?fn\s+").is_match(next_code) {
                scan.add(
                    line_number,
                    "P-STALE-ALLOW-LINT",
                    "Suppressed Dead Code Without Expectation",
                    Severity::Medium,
                    "Function suppressed using bare '#[allow(dead_code)]'. Dead functions mask incomplete features or abandoned stubs.",
                    "Either add tests/callers, delete unused code, or migrate to '#[expect(dead_code, reason = \"...\")]' if temporarily required.",
                );
            }
        }
        if !scan.test_file
            && !scan.build_file
            && test_scopes.is_empty()
            && !pending_test
            && !test_attribute
            && trigger_candidates
                .iter()
                .any(|position| *position >= line_start && *position < offset)
        {
            scan.add(line_number, "P-TRIGGER-ROW-CHANGE", "Column-Filtered Update Trigger", Severity::High,
                "Column-filtered AFTER UPDATE trigger may miss changes made by a BEFORE trigger. Check the active migration chain and database semantics.",
                "If another trigger derives a watched column, observe row changes with the owner-approved trigger predicate.");
        }

        if !scan.test_file
            && !scan.build_file
            && !scan.vendored
            && test_scopes.is_empty()
            && !pending_test
            && !test_attribute
        {
            let marker = re!(r"\b(?:todo|unimplemented)\s*!\s*[(\[{]")
                .find(code_part)
                .or_else(|| {
                    re!(r"(?i)\b(?:FIXME|TODO|XXX)\b|in production you'd|not production(?: ready)?|dummy implementation|stub for now").find(&comment)
                });
            if let Some(marker) = marker {
                scan.add(
                    line_number,
                    "P-UNFINISHED-CODE",
                    "Lingering Incomplete Marker",
                    Severity::Medium,
                    format!(
                        "Found unfinished work marker '{}'. Check the reachable behavior.",
                        marker.as_str()
                    ),
                    "Implement required behavior and track agreed follow-up in the owner plan.",
                );
                scan.findings.last_mut().expect("finding exists").content =
                    scan.lines[index].original.trim().to_owned();
            }
        }
        if code_part.is_empty() {
            continue;
        }
        let opens = code_part.bytes().filter(|byte| *byte == b'{').count();
        let closes = code_part.bytes().filter(|byte| *byte == b'}').count();
        if test_attribute {
            pending_test = true;
        }
        let test_context = scan.test_file || !test_scopes.is_empty() || pending_test;
        if pending_test {
            if opens > 0 {
                test_scopes.push(depth);
                pending_test = false;
            } else if code_part.contains(';') {
                pending_test = false;
            }
        }
        if re!(r"\b(?:for\s+[^;{]+?\s+in\b|while\b)").is_match(code_part) {
            if opens > 0 {
                loop_depths.push(depth);
            } else {
                pending_loop = true;
            }
        } else if pending_loop && opens > 0 {
            loop_depths.push(depth);
            pending_loop = false;
        }
        let static_decl = re!(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:static|const)\b")
            .is_match(code_part)
            || code_part.contains("Lazy::new")
            || code_part.contains("LazyLock::new")
            || code_part.contains("lazy_static!");
        if static_decl || pending_static {
            if !pending_static {
                static_delimiters.clear();
            }
            let mut terminated = false;
            for byte in code_part.bytes() {
                match byte {
                    b'(' | b'[' => static_delimiters.push(byte),
                    b')' if static_delimiters.last() == Some(&b'(') => {
                        static_delimiters.pop();
                    }
                    b']' if static_delimiters.last() == Some(&b'[') => {
                        static_delimiters.pop();
                    }
                    b';' if static_delimiters.is_empty() => {
                        terminated = true;
                        break;
                    }
                    _ => {}
                }
            }
            if opens > 0 {
                static_depth = Some(depth);
                pending_static = false;
            } else {
                pending_static = !terminated;
            }
        }

        if let Some(captures) =
            re!(r"\b(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z_]\w*)")
                .captures(code_part)
        {
            let fn_name = &captures[1];
            if fn_name != "from"
                && !fn_name.starts_with("from_")
                && let Some(paren_pos) = code_part.find('(')
            {
                let after_paren = &code_part[paren_pos..];
                let open_p = after_paren.bytes().filter(|b| *b == b'(').count();
                let close_p = after_paren.bytes().filter(|b| *b == b')').count();
                if open_p > close_p {
                    in_fn_params = true;
                    fn_param_parens = open_p - close_p;
                }
                if !test_context && !scan.test_file && !scan.build_file {
                    let ptr_arg = re!(
                        r":\s*(?:Option\s*<\s*)?&(?:String|Vec\s*<|PathBuf)\b|&\[\s*&String\s*\]"
                    );
                    if ptr_arg.is_match(after_paren) {
                        scan.add(line_number, "P-PTR-ARG", "Overly Restrictive Pointer Argument", Severity::High,
                            "Function takes an immutable reference to an owned container ('&String', '&Vec', or '&PathBuf') instead of a borrowed slice ('&str', '&[T]', or '&Path').",
                            "Change parameter to a borrowed slice ('&str', '&[T]', or '&Path') to accept both borrowed and owned callers without unnecessary allocations.");
                    }
                }
            }
        } else if in_fn_params {
            let open_p = code_part.bytes().filter(|b| *b == b'(').count();
            let close_p = code_part.bytes().filter(|b| *b == b')').count();
            if !test_context && !scan.test_file && !scan.build_file {
                let ptr_arg =
                    re!(r":\s*(?:Option\s*<\s*)?&(?:String|Vec\s*<|PathBuf)\b|&\[\s*&String\s*\]");
                if ptr_arg.is_match(code_part) {
                    scan.add(line_number, "P-PTR-ARG", "Overly Restrictive Pointer Argument", Severity::High,
                        "Function takes an immutable reference to an owned container ('&String', '&Vec', or '&PathBuf') instead of a borrowed slice ('&str', '&[T]', or '&Path').",
                        "Change parameter to a borrowed slice ('&str', '&[T]', or '&Path') to accept both borrowed and owned callers without unnecessary allocations.");
                }
            }
            if fn_param_parens + open_p <= close_p {
                in_fn_params = false;
                fn_param_parens = 0;
            } else {
                fn_param_parens = (fn_param_parens + open_p) - close_p;
            }
        }

        if let Some(captures) =
            re!(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?async\s+fn\s+([a-zA-Z_]\w*)").captures(code_part)
        {
            let function = &captures[1];
            if opens > 0 {
                async_depth = Some(depth);
            } else {
                pending_async = true;
            }
            if scan.services
                && !test_context
                && code_part.starts_with("pub")
                && re!(
                    r"^(?:create|update|delete|set|reconcile|merge|publish|execute|handle|dispatch)"
                )
                .is_match(function)
                && !scan.lines[index.saturating_sub(15)..index]
                    .iter()
                    .any(|previous| previous.syntax.contains("instrument"))
            {
                scan.add(line_number, "P-STRUCTURED-TRACE", "Contextual Trace Spans", Severity::Medium,
                    format!("Candidate mutation '{function}' has no nearby instrumentation marker."),
                    "Verify that the owner entry point has the required operation and tenant context.");
            }
        } else if pending_async && opens > 0 {
            async_depth = Some(depth);
            pending_async = false;
        }
        if async_depth.is_some() && !test_context {
            if re!(r"\bstd::thread::sleep\s*\(").is_match(code_part) {
                scan.add(
                    line_number,
                    "P-SYNC-SLEEP-AWAIT",
                    "Sync Sleep in Async Code",
                    Severity::Critical,
                    "Blocking sleep appears in an async function. Verify which thread executes it.",
                    "Use tokio::time::sleep(duration).await on executor workers.",
                );
            }
            let blocking_fs = re!(
                r"\bstd::fs::(?:read|read_to_string|write|remove_file|remove_dir|remove_dir_all|create_dir|create_dir_all|copy|rename|File::open|File::create)\s*\("
            );
            if blocking_fs.is_match(code_part) {
                scan.add(
                    line_number,
                    "P-BLOCKING-IO-IN-ASYNC",
                    "Blocking I/O in Async Code",
                    Severity::High,
                    "Synchronous std::fs I/O appears in an async function. This blocks the Tokio executor worker thread.",
                    "Use tokio::fs or execute blocking filesystem operations inside tokio::task::spawn_blocking.",
                );
            }
            if let Some(captures) = re!(r"\blet\s+(?:mut\s+)?([a-zA-Z_]\w*)\s*=\s*([^;\n]+?\.(?:lock|read|write)\s*\([^;\n]*\))").captures(code_part) {
                let acquisition = scan.lines[index..(index + 6).min(scan.lines.len())].iter().map(|line| line.syntax.trim()).collect::<Vec<_>>().join(" ");
                if !code_part.contains("tokio::sync") && !acquisition.split(';').next().unwrap_or("").contains(".await") {
                    guards.insert(captures[1].to_owned(), (line_number, depth));
                }
            }
            if let Some(captures) =
                re!(r"^(?:std::mem::)?drop\s*\(\s*([A-Za-z_]\w*)\s*\)\s*;").captures(code_part)
            {
                if guards
                    .get(&captures[1])
                    .is_some_and(|guard| guard.1 == depth)
                {
                    guards.remove(&captures[1]);
                }
            }
            if re!(r"\.await\b").is_match(code_part) {
                let active: Vec<_> = guards
                    .iter()
                    .filter(|(_, guard)| depth >= guard.1)
                    .map(|(name, guard)| (name.clone(), *guard))
                    .collect();
                for (name, (acquired, _)) in active {
                    scan.add(line_number, "P-SYNC-LOCK-AWAIT", "Sync Lock Held Across Await", Severity::Critical,
                        format!("Possible synchronous guard '{name}' from line {acquired} across .await."),
                        format!("Verify the guard type and end '{name}' scope before awaiting when possible."));
                    guards.remove(&name);
                }
            }
        }

        check_rules(
            &mut scan,
            RuleContext {
                index,
                test_context,
                loop_depth: loop_depths.last().copied(),
                static_depth,
                async_depth,
                depth,
                statement_offset,
                line_start,
                line_end: offset,
                reactive: &reactive,
                publications: &publications,
                has_unprepared_sql,
            },
            &mut sql_bindings,
        );

        depth = depth.saturating_add(opens).saturating_sub(closes);
        guards.retain(|_, guard| guard.1 <= depth);
        sql_bindings.retain(|binding_depth, _| *binding_depth <= depth);
        while test_scopes.last().is_some_and(|scope| depth <= *scope) {
            test_scopes.pop();
        }
        while loop_depths.last().is_some_and(|scope| depth <= *scope) {
            loop_depths.pop();
        }
        if async_depth.is_some_and(|scope| depth <= scope) {
            async_depth = None;
            guards.clear();
        }
        if static_depth.is_some_and(|scope| depth <= scope) {
            static_depth = None;
        }
    }
    scan.findings
}

struct RuleContext<'a> {
    index: usize,
    test_context: bool,
    loop_depth: Option<usize>,
    static_depth: Option<usize>,
    async_depth: Option<usize>,
    depth: usize,
    statement_offset: usize,
    line_start: usize,
    line_end: usize,
    reactive: &'a [usize],
    publications: &'a [usize],
    has_unprepared_sql: bool,
}

fn check_rules(
    scan: &mut Scan<'_>,
    context: RuleContext<'_>,
    sql_bindings: &mut BTreeMap<usize, BTreeMap<String, bool>>,
) {
    let RuleContext {
        index,
        test_context,
        loop_depth,
        static_depth,
        async_depth,
        depth,
        statement_offset,
        line_start,
        line_end,
        reactive,
        publications,
        has_unprepared_sql,
    } = context;
    let number = index + 1;
    let code = scan.lines[index].code.clone();
    let syntax = scan.lines[index].syntax.clone();
    let text = syntax.trim();

    let include = re!(r#"\binclude\s*!\s*\(\s*"([^"\\]+\.rs)"\s*\)"#);
    if !test_context && !scan.build_file && syntax.contains("include") {
        let end = scan.lines.len();
        let code_window = scan.lines[index..end]
            .iter()
            .map(|line| line.code.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let syntax_window = scan.lines[index..end]
            .iter()
            .map(|line| line.syntax.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(found) =
            matched_with_literals(include, &code_window, &syntax_window).filter(|found| {
                found
                    .get(0)
                    .is_some_and(|matched| matched.start() < code.len())
            })
        {
            let included = &found[1];
            let module = Path::new(included)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or(included);
            scan.add(number, "P-ZERO-INCLUDE", "Zero Include Slop", Severity::High,
                format!("Forbidden file-stitching macro '{}'. Use canonical 'mod ...;' and public exports.", &found[0]),
                format!("Declare mod {module}; and expose only the required items."));
        }
    }

    if !test_context {
        if re!(r"^\s*(?:[A-Za-z_]\w*\s*\.\s*)?(?:transaction|txn|tx)\b").is_match(text) {
            let continuation_of_discarded = scan.lines[..index]
                .iter()
                .rev()
                .take(5)
                .take_while(|line| !line.syntax.contains(';'))
                .any(|line| re!(r"^\s*let\s+_\s*=").is_match(&line.syntax));
            let window = scan.lines[index..(index + 12).min(scan.lines.len())]
                .iter()
                .map(|line| line.syntax.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if let Some(capture) = re!(r"\b(?:transaction|txn|tx)\s*\.\s*(rollback|commit)\s*\(\s*\)\s*\.\s*await\s*\.\s*ok\s*\(\s*\)")
                .captures(&window).filter(|capture| capture.get(0).is_some_and(|found| found.start() < syntax.len())) {
                if !continuation_of_discarded {
                let operation = &capture[1];
                scan.add(number, "P-TXN-ROLLBACK-SWALLOW", "Transaction Error Converted to Option", Severity::Critical,
                    format!("{operation} error is converted to Option. Check whether the failure is observable and affects recovery."),
                    "Handle the finalization error while preserving the primary error and recovery contract.");
                }
            }
        }
        if syntax.contains("serde_json") {
            let window = scan.lines[index..(index + 16).min(scan.lines.len())]
                .iter()
                .map(|line| line.syntax.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if json_decode_defaults(&window, syntax.len()) {
                scan.add(number, "P-JSON-DECODE-DEFAULT", "JSON Decode Failure Defaulted", Severity::High,
                    "JSON decode failure becomes a default value. Verify compatibility policy and whether invalid stored data must be surfaced.",
                    "Propagate a typed decode error, or explicitly handle and observe a documented compatibility fallback.");
            }
        }
        if syntax.contains("filter_map") {
            let window = scan.lines[index..(index + 16).min(scan.lines.len())]
                .iter()
                .map(|line| line.syntax.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if filter_discards_errors(&window, syntax.len()) {
                scan.add(number, "P-FILTER-DROPPED-ERROR", "Fallible Items Silently Dropped", Severity::High,
                    "filter_map converts item errors to None. Check whether partial results or lost diagnostics are allowed by the owner.",
                    "Propagate item errors or explicitly record and handle rejected items under the owner contract.");
            }
        }
        if syntax.contains("spawn") {
            let window = scan.lines[index..(index + 128).min(scan.lines.len())]
                .iter()
                .map(|line| line.syntax.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if orphan_task_spawn(&window, syntax.len()) {
                scan.add(number, "P-ORPHAN-SPAWN", "Unsupervised Task Spawn", Severity::High,
                    "Unsupervised background task spawned without retaining its JoinHandle. Panics are silently swallowed and the task will not participate in lifecycle shutdown.",
                    "Bind to a JoinHandle, store in a JoinSet or task supervisor, or pass a CancellationToken to manage task lifecycle.");
            }
        }
        if syntax.contains("set_len") && !scan.build_file {
            let set_len = re!(r"\b([A-Za-z_]\w*)\s*\.\s*set_len\s*\(");
            if let Some(captures) = set_len.captures(text) {
                let var = &captures[1];
                scan.add(number, "P-UNSAFE-VEC-SETLEN", "Unchecked Vector Length Mutation", Severity::Critical,
                    format!("Direct invocation of 'set_len' on '{var}' exposes uninitialized memory if capacity or bounds diverge."),
                    "Use resize, extend, or validate length <= capacity() with explicit memory initialization before setting length.");
            }
        }
        if re!(r"^\s*let\s+_\s*=").is_match(text) {
            let mut statement = text.to_owned();
            if !statement.contains(';') {
                for following in scan.lines.iter().skip(index + 1).take(5) {
                    let next = following.syntax.trim();
                    if next.starts_with("pub ")
                        || next.starts_with("fn ")
                        || next.starts_with("async ")
                    {
                        break;
                    }
                    statement.push(' ');
                    statement.push_str(next);
                    if next.contains(';') {
                        break;
                    }
                }
            }
            let propagates = discarded_result_propagates(&statement);
            if !propagates {
                if let Some(capture) = re!(r"\blet\s+_\s*=\s*(?:\w+\.)?(?:transaction|txn|tx)\s*\.\s*(rollback|commit)\s*\(\s*\)\s*\.\s*await\b").captures(&statement) {
                let operation = &capture[1];
                let fix = if operation == "commit" {
                    "Propagate the commit failure and preserve the operation's retry contract."
                } else {
                    "Handle the rollback failure while preserving the primary operation error."
                };
                scan.add(number, "P-TXN-ROLLBACK-SWALLOW", "Ignored Transaction Finalization Result", Severity::Critical,
                    format!("Ignored {operation} result on a transaction-like value."), fix);
            } else if re!(r"let\s+_\s*=\s*(?:\w+_)?(?:nonce|signal|state)\.get\(\);").is_match(&statement) {
                let boundaries_before = reactive.partition_point(|boundary| *boundary <= statement_offset);
                if boundaries_before % 2 == 0 {
                    scan.add(number, "P-LEPTOS-DEAD-SIGNAL", "Dead Reactive Signal Read", Severity::High,
                        "Discarded signal read outside a recognized reactive scope. Verify the actual reactive consumer.",
                        "If tracking is required, read the signal in the owning reactive consumer.");
                }
            } else if re!(r"\blet\s+_\s*=\s*(?:(?:tokio|std\s*::\s*thread)\s*::\s*(?:task\s*::\s*)?)?spawn\s*\(").is_match(&statement) {
                // Handled directly by P-ORPHAN-SPAWN above
            } else if re!(r"\b(?:\w+\s*\(|await\b)").is_match(&statement) {
                scan.add(number, "P-ANTI-SWALLOW", "Zero Error Swallowing", Severity::High,
                    "Discarded call result via 'let _ ='. Verify whether it can fail.",
                    "Propagate a required failure or record an explicitly best-effort failure.");
            }
            }
        }
        if re!(r"\b(?:std::hint::)?black_box\s*\(").is_match(text) {
            scan.add(number, "P-ANTI-SWALLOW", "Dummy Compiler Suppression", Severity::High,
                "Abuse of 'black_box' in production service code. Do not fake variable usage to silence linter.",
                "Remove black_box and handle or return the underlying value.");
        }
    }

    if !test_context && !scan.build_file {
        let static_item = static_depth.is_some_and(|scope| depth >= scope)
            || re!(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:static|const)\b").is_match(text)
            || text.contains("Lazy::new")
            || text.contains("LazyLock::new")
            || text.contains("lazy_static!");

        if (async_depth.is_some() || loop_depth.is_some()) && !static_item {
            let client_alloc = re!(
                r"\b(?:reqwest\s*::\s*)?Client\s*::\s*new\s*\(\s*\)|\b(?:reqwest\s*::\s*)?ClientBuilder\s*::\s*new\s*\(\s*\)|\b(?:tokio\s*::\s*net\s*::\s*|std\s*::\s*net\s*::\s*)?UdpSocket\s*::\s*bind\s*\("
            );
            if client_alloc.is_match(text) {
                scan.add(number, "P-CLIENT-PER-REQUEST", "Per-Request Client / Socket Allocation", Severity::High,
                    "Instantiating an HTTP client or socket per async call or in a loop risks socket exhaustion and destroys connection pooling.",
                    "Instantiate once in a long-lived struct, shared state, or static LazyLock and reuse it across requests.");
            }
        }

        if syntax.contains("danger") || syntax.contains("assertion") {
            let tls_bypass = re!(
                r"\b(?:rustls\s*::\s*(?:\w+\s*::\s*)*danger\b|(?:HandshakeSignatureValid|ServerCertVerified)\s*::\s*assertion\s*\(\s*\)|danger_accept_invalid_certs\s*\(\s*true\s*\)|danger_accept_invalid_hostnames\s*\(\s*true\s*\))"
            );
            if let Some(matched) = tls_bypass.find(text) {
                scan.add(number, "P-TLS-DANGER-BYPASS", "Disabled TLS Certificate Verification", Severity::Critical,
                    format!("Disabled TLS certificate or signature verification ('{}'). This bypasses MITM protection.", matched.as_str()),
                    "Verify certificates using trusted root authorities rather than asserting validity.");
            }
        }

        if syntax.contains("from_be_bytes") || syntax.contains("from_le_bytes") {
            let window: Vec<&str> = scan.lines[index..(index + 10).min(scan.lines.len())]
                .iter()
                .map(|l| l.syntax.as_str())
                .collect();
            if let Some(var) = unbounded_network_allocation(&window) {
                scan.add(number, "P-UNBOUNDED-ALLOC-OOM", "Unbounded Network Buffer Allocation", Severity::Critical,
                    format!("Vector allocated with length from '{var}' without an upper bound check. Risks Out-Of-Memory panic on malformed headers."),
                    "Validate length <= MAX_PAYLOAD_BYTES (or ensure_frame_size) before allocating the buffer.");
            }
        }

        if re!(r"\.(?:expect|unwrap)\s*\(").is_match(text)
            && !static_item
            && !(text.contains("Regex::new") && (text.contains("static") || text.contains("const")))
        {
            scan.add(number, "P-ZERO-PANIC-PROD", "Zero Production Panics", Severity::High,
                "Calling .expect() or .unwrap() in production path. Return typed DomainError / PortError.",
                "Use '?' operator with .map_err(DomainError::from) or return Result.");
        }
        if re!(r"\.unwrap_err\s*\(").is_match(text) {
            scan.add(
                number,
                "P-ZERO-PANIC-PROD",
                "Zero Production Panics",
                Severity::High,
                "Calling .unwrap_err() in production path. Match on Result explicitly.",
                "match result { Err(e) => ..., Ok(_) => return Err(...) }",
            );
        }
        if re!(r"\bpanic!\s*\(").is_match(text) {
            scan.add(
                number,
                "P-ZERO-PANIC-PROD",
                "Zero Production Panics",
                Severity::High,
                "Calling panic!(...) in production path. Return typed DomainError / PortError.",
                "return Err(DomainError::from(...));",
            );
        }
    }

    if !test_context && (re!(r"\b(?:[A-Z0-9_]+_)?SHORT_ID_LEN(?:GTH)?\s*(?::\s*usize)?\s*=\s*12\b|\.\s*simple\s*\(\s*\)\s*\.\s*to_string\s*\(\s*\)\s*\[\s*0?\s*\.\.\s*12\s*\]").is_match(text)
        || re!(r#"(?i)\b(?:route_)?short_(?:id|identity)"?\s+VARCHAR\s*\(\s*12\s*\)"#).is_match(&code))
    {
        scan.add(number, "P-ID-ENTROPY", "Bounded Identifier Entropy", Severity::Medium,
            "Possible 12-hex identifier. Verify generator entropy and the uniqueness scope.",
            "Use an owner-defined collision policy and preserve existing route compatibility.");
    }
    if !test_context {
        let mut arm = text.to_owned();
        if text.contains("PermissionScope::None")
            && text.contains("=>")
            && !text.contains(';')
            && !text.contains('}')
        {
            for following in scan.lines.iter().skip(index + 1).take(3) {
                let next = following.syntax.trim();
                arm.push(' ');
                arm.push_str(next);
                if next.contains(';') || next.contains('}') || next.contains(',') {
                    break;
                }
            }
        }
        let success = re!(
            r"\bPermissionScope::None\s*=>\s*(?:\{\s*)?(?:return\s+)?(?:Ok\s*\(\s*(?:true\b|\(\s*\))\s*\)|true\b)"
        );
        if success.find(&arm).is_some_and(|found| {
            let rest = arm[found.end()..].trim_start();
            rest.is_empty()
                || rest
                    .chars()
                    .next()
                    .is_some_and(|character| matches!(character, ',' | ';' | '}'))
        }) {
            scan.add(
                number,
                "P-FAIL-CLOSED",
                "Fail-Closed Security Floor",
                Severity::Critical,
                "PermissionScope::None may grant access. Verify the caller's decision contract.",
                "Return the owner-defined denial value at the authorization boundary.",
            );
        }
    }

    if !test_context && (scan.error_file || scan.services) {
        let direct = re!(
            r#"\b(?:err|error|db_err|sql_err|query_err)\s*(?:\.\s*to_string\s*\(\s*\))?\s*\.\s*contains\s*\(\s*"([^"]+)""#
        );
        let generic = re!(
            r#"\b(?:message|msg|e)\s*(?:\.\s*to_string\s*\(\s*\))?\s*\.\s*contains\s*\(\s*"([^"]+)""#
        );
        let hint = re!(
            r#"(?i)(?:duplicate|unique|violates|constraint|foreign\s*key|not-null|23505|23503|23502|23514|42P01|P0001|deadlock|syntax\s*error|relation\s*"\w+"\s*does\s*not\s*exist)"#
        );
        if matched_with_literals(direct, &code, &syntax).is_some()
            || matched_with_literals(generic, &code, &syntax)
                .is_some_and(|captures| hint.is_match(&captures[1]))
        {
            scan.add(number, "P-SQLSTATE-DISCRIM", "SQLSTATE Error Mapping", Severity::High,
                "Brittle string matching on error message. Discriminate using SQLSTATE (23505, P0001).",
                "Match on db_err.sql_state() == Some(\"23505\") or \"P0001\".");
        }
    }
    if loop_depth.is_some() && !test_context
        && re!(r"\.(?:one|all|one_by_id|query_one|query_all)\s*\(\s*&?(?:(?:\w+\.)?\w*(?:txn|tx|transaction|db|conn|pool)\w*)\s*\)\.await").is_match(text)
    {
        scan.add(number, "P-BATCH-QUERY", "Transactional Batching", Severity::High,
            "Suspected N+1 query: database query inside loop body. Batch with Column::Id.is_in(ids).all().",
            "Collect IDs and perform one Entity::find().filter(Column::Id.is_in(ids)).all(txn) before loop.");
    }
    if !test_context
        && !scan.migration
        && re!(r"^#!\s*\[\s*allow\s*\([^)]*\b(?:dead_code|unused|unused_variables)\b")
            .is_match(text)
    {
        scan.add(
            number,
            "P-DEAD-TWIN",
            "File-Level Slop Suppression",
            Severity::Medium,
            "Module-wide dead-code or unused lint suppression may hide unused implementations.",
            "Verify reachability and remove dormant implementations.",
        );
    }
    if has_unprepared_sql
        && !test_context
        && !scan.migration
        && (syntax.contains("execute_unprepared") || text.contains('='))
    {
        let end = (index + 16).min(scan.lines.len());
        let code_window = scan.lines[index..end]
            .iter()
            .map(|line| line.code.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let syntax_window = scan.lines[index..end]
            .iter()
            .map(|line| line.syntax.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if sql::check(&code_window, &syntax_window, depth, sql_bindings) {
            scan.add(number, "P-RAW-SQL-INTERP", "Unprepared SQL Interpolation", Severity::Critical,
                "Unprepared SQL contains formatted values. Verify their origin and parameterize query data.",
                "Statement::from_sql_and_values(db.get_database_backend(), sql, values)");
        }
    }
    if !test_context {
        let tenant = re!(
            r"\btenant(?:_id)?(?:\s*\.\s*(?:as_ref|as_deref|copied|cloned)\s*\(\s*\))*\s*\.\s*unwrap_or(?:_default\s*\(\s*\)|_else\s*\(\s*(?:\|\|\s*)?(?:Uuid::nil|Uuid::default|Default::default)\s*(?:\(\s*\))?\s*\)|\s*\(\s*(?:Uuid::nil|Uuid::default|Default::default)\s*\(\s*\)\s*\))"
        );
        let binding = re!(
            r"^\s*let\s+(?:mut\s+)?tenant_id(?:\s*:\s*[^=;]+)?\s*=\s*(?:[A-Za-z_]\w*\s*\.\s*)*[A-Za-z_]\w*(?:\s*\.\s*(?:as_ref|as_deref|copied|cloned)\s*\(\s*\))*\s*\.\s*unwrap_or(?:_default\s*\(\s*\)|_else\s*\(\s*(?:\|\|\s*)?(?:Uuid::nil|Uuid::default|Default::default)\s*(?:\(\s*\))?\s*\)|\s*\(\s*(?:Uuid::nil|Uuid::default|Default::default)\s*\(\s*\)\s*\))"
        );
        if tenant.is_match(text) || binding.is_match(text) {
            scan.add(number, "P-TENANT-FALLBACK-LEAK", "Tenant Fallback Leakage", Severity::Critical,
                "Possible default tenant selector. Verify that missing tenant authority fails closed.",
                "tenant_id.ok_or(DomainError::TenantNotFound)?");
        }
        if publications
            .iter()
            .any(|offset| *offset >= line_start && *offset < line_end)
        {
            scan.add(number, "P-OUTBOX-DETACHED", "Transactional Outbox Decoupling", Severity::High,
                "Direct publication in a function with transaction evidence. Verify which mutation the event represents.",
                "Write events for the mutation through the owner outbox in the same transaction.");
        }
    }
    if scan.services && !test_context
        && re!(r"\b(?:let\s+(?:mut\s+)?\w+\s*=|if\s+let\s+Some\(\w+\)\s*=)\s*input\.\w+\.(?:clone\(\)|to_string\(\))").is_match(text)
        && scan.lines.iter().skip(index + 1).take(5).any(|next| next.syntax.contains("validate_") || next.syntax.contains("check_"))
    {
        scan.add(number, "P-HOTPATH-ALLOC", "Premature Input Allocation", Severity::Medium,
            "Cloning input field immediately before a validation check. Validate borrowed reference first.",
            "Validate with &input.field or .as_deref() first, clone only after validation passes.");
    }
}
