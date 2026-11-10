use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::{Confidence, Finding, Severity};

fn severity(value: Severity) -> &'static str {
    match value {
        Severity::Critical => "P0-CRITICAL",
        Severity::High => "P1-HIGH",
        Severity::Medium => "P2-MEDIUM",
    }
}

fn confidence(value: Confidence) -> &'static str {
    match value {
        Confidence::High => "high",
        Confidence::Heuristic => "heuristic",
    }
}

fn groups(findings: &[Finding]) -> BTreeMap<&str, Vec<&Finding>> {
    let mut result: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for finding in findings {
        result.entry(finding.pattern_id).or_default().push(finding);
    }
    result
}

fn module(path: &str) -> &str {
    let parts: Vec<_> = path.split('/').collect();
    parts
        .windows(2)
        .find(|pair| {
            matches!(
                pair[0],
                "modules" | "libs" | "workers" | "apps" | "ui" | "tools" | "utils"
            )
        })
        .map_or("root", |pair| pair[1])
}

fn scorecard(findings: &[Finding]) -> Vec<(&str, [usize; 5])> {
    let mut counts = BTreeMap::<&str, [usize; 5]>::new();
    for finding in findings {
        let values = counts.entry(module(&finding.file)).or_default();
        values[match finding.severity {
            Severity::Critical => 0,
            Severity::High => 1,
            Severity::Medium => 2,
        }] += 1;
        values[if finding.confidence == Confidence::High {
            3
        } else {
            4
        }] += 1;
    }
    let mut result: Vec<_> = counts.into_iter().collect();
    result.sort_by(|a, b| {
        b.1[3]
            .cmp(&a.1[3])
            .then(b.1[0].cmp(&a.1[0]))
            .then(b.1[1].cmp(&a.1[1]))
            .then(a.0.cmp(b.0))
    });
    result
}

pub(crate) fn print(
    findings: &[Finding],
    count: usize,
    target: &str,
    summary_only: bool,
    suggestions: bool,
    score: bool,
) {
    println!("======================================================================");
    println!(" RusToK Domain Hygiene & Anti-Slop Scanner");
    println!(" Canonical Reference: docs/standards/logic-hygiene-patterns.md");
    println!(" Target: {target}");
    println!("======================================================================");
    println!(
        "Scanned {count} Rust file(s). Found {} issue(s).\n",
        findings.len()
    );
    if findings.is_empty() {
        println!("No findings for the selected rules and source files.");
        return;
    }
    if summary_only || score || count > 30 {
        println!("\nPLATFORM HYGIENE SCORECARD (BY MODULE)");
        println!(
            "{:<34} | {:>4} | {:>4} | {:>4} | {:>15} | Heuristic",
            "Module / Component", "P0", "P1", "P2", "High confidence"
        );
        for (name, values) in scorecard(findings) {
            println!(
                "{name:<34} | {:>4} | {:>4} | {:>4} | {:>15} | {}",
                values[0], values[1], values[2], values[3], values[4]
            );
        }
    }
    if summary_only {
        return;
    }
    for (pattern, items) in groups(findings) {
        println!(
            "=== [{}] {pattern}: {} ({} occurrence(s)) ===",
            severity(items[0].severity),
            items[0].rule_name,
            items.len()
        );
        for finding in items.iter().take(10) {
            println!(
                "  {}:{} [{}]",
                finding.file,
                finding.line,
                confidence(finding.confidence)
            );
            println!("    Line:    {}", finding.content);
            println!("    Fix:     {}", finding.message);
            if suggestions {
                if let Some(fix) = &finding.suggested_fix {
                    println!("    Suggest: {fix}");
                }
            }
            println!();
        }
        if items.len() > 10 {
            println!("  ... and {} more occurrences omitted.\n", items.len() - 10);
        }
    }
}

pub(crate) fn csv(findings: &[Finding]) -> Result<String, String> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "pattern_id",
            "rule_name",
            "severity",
            "file",
            "line",
            "snippet",
            "message",
            "suggested_fix",
            "confidence",
        ])
        .map_err(|error| error.to_string())?;
    for finding in findings {
        writer
            .write_record([
                finding.pattern_id,
                finding.rule_name,
                severity(finding.severity),
                &finding.file,
                &finding.line.to_string(),
                finding.content.trim(),
                &finding.message,
                finding.suggested_fix.as_deref().unwrap_or(""),
                confidence(finding.confidence),
            ])
            .map_err(|error| error.to_string())?;
    }
    String::from_utf8(writer.into_inner().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

pub(crate) fn markdown(findings: &[Finding], scanned: usize, target: &str) -> String {
    let high = findings
        .iter()
        .filter(|finding| finding.confidence == Confidence::High)
        .count();
    let mut lines = vec![
        "# RusToK Domain Hygiene & Invariant Verification Report".to_owned(),
        String::new(),
        format!("- **Target:** `{target}`"),
        format!("- **Files Scanned:** {scanned}"),
        format!(
            "- **Findings:** {} ({high} high confidence, {} heuristic)",
            findings.len(),
            findings.len() - high
        ),
        String::new(),
        "## Module Scorecard".to_owned(),
        String::new(),
        "| Module | P0 Critical | P1 High | P2 Medium | High confidence | Heuristic |".to_owned(),
        "|---|---|---|---|---|---|".to_owned(),
    ];
    for (name, values) in scorecard(findings) {
        lines.push(format!(
            "| `{name}` | {} | {} | {} | {} | {} |",
            values[0], values[1], values[2], values[3], values[4]
        ));
    }
    lines.extend([
        String::new(),
        "## Discovered Findings by Pattern".to_owned(),
        String::new(),
    ]);
    for (pattern, items) in groups(findings) {
        lines.push(format!(
            "### `[{}]` {pattern}: {} ({} occurrence(s))",
            severity(items[0].severity),
            items[0].rule_name,
            items.len()
        ));
        lines.extend([
            String::new(),
            "| File | Line | Confidence | Snippet | Recommendation |".to_owned(),
            "|---|---|---|---|---|".to_owned(),
        ]);
        for finding in items.iter().take(15) {
            let snippet = finding.content.replace('|', "\\|").replace('`', "'");
            let snippet: String = snippet.chars().take(60).collect();
            let fix = finding
                .suggested_fix
                .as_deref()
                .unwrap_or(&finding.message)
                .replace('|', "\\|")
                .replace('`', "'");
            lines.push(format!(
                "| `{}:{}` | {} | {} | `{}` | {} |",
                finding.file,
                finding.line,
                finding.line,
                confidence(finding.confidence),
                snippet.trim(),
                fix
            ));
        }
        if items.len() > 15 {
            lines.push(format!(
                "| *... and {} more occurrences* | | | | |",
                items.len() - 15
            ));
        }
        lines.push(String::new());
    }
    lines.push("---".to_owned());
    lines.push("Canonical Standards Reference: [`docs/standards/logic-hygiene-patterns.md`](docs/standards/logic-hygiene-patterns.md)".to_owned());
    lines.join("\n")
}

fn diff_path(prefix: &str, path: &str) -> String {
    let name = format!("{prefix}/{path}");
    if !name
        .bytes()
        .any(|byte| byte < 32 || byte == b'"' || byte == b'\\')
    {
        return name;
    }
    let mut quoted = String::from("\"");
    for byte in name.bytes() {
        if (32..127).contains(&byte) && byte != b'"' && byte != b'\\' {
            quoted.push(char::from(byte));
        } else {
            quoted.push_str(&format!("\\{byte:03o}"));
        }
    }
    quoted.push('"');
    quoted
}

pub(crate) fn patch(
    findings: &[Finding],
    sources: &BTreeMap<PathBuf, String>,
) -> Result<String, String> {
    let mut files: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for finding in findings
        .iter()
        .filter(|finding| finding.suggested_fix.is_some())
    {
        files.entry(&finding.file).or_default().push(finding);
    }
    let mut patch = String::new();
    for (file, mut findings) in files {
        let path = Path::new(file);
        if path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
            return Err(format!(
                "--patch requires files inside the repository: {file}"
            ));
        }
        let source = sources
            .get(path)
            .ok_or_else(|| format!("missing patch source: {file}"))?;
        let newline = if source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let lines: Vec<_> = source.split_inclusive('\n').collect();
        let lacks_newline = !source.ends_with('\n');
        let original_count = lines.len();
        let added_count = findings.len() + 1;
        let new_count = original_count + added_count;
        patch.push_str(&format!(
            "--- {}\n+++ {}\n@@ -1,{original_count} +1,{new_count} @@\n",
            diff_path("a", file),
            diff_path("b", file)
        ));
        for (index, line) in lines.iter().enumerate() {
            if lacks_newline && index + 1 == lines.len() {
                patch.push('-');
                patch.push_str(line);
                patch.push_str("\n\\ No newline at end of file\n+");
                patch.push_str(line);
                patch.push_str(newline);
            } else {
                patch.push(' ');
                patch.push_str(line);
            }
        }
        patch.push('+');
        patch.push_str(newline);
        findings.sort_by_key(|finding| finding.line);
        for finding in findings {
            let fix = finding
                .suggested_fix
                .as_deref()
                .unwrap_or("")
                .lines()
                .collect::<Vec<_>>()
                .join(" ");
            patch.push_str(&format!(
                "+// REMEDIATION ({}, line {}): {}{}",
                finding.pattern_id, finding.line, fix, newline
            ));
        }
    }
    Ok(patch)
}

pub(crate) fn annotations(findings: &[Finding]) {
    for finding in findings {
        let level =
            if finding.confidence == Confidence::High && finding.severity == Severity::Critical {
                "error"
            } else if finding.severity <= Severity::High {
                "warning"
            } else {
                "notice"
            };
        let message = finding
            .message
            .replace('%', "%25")
            .replace('\n', " ")
            .replace('\r', "");
        let file = finding
            .file
            .replace('%', "%25")
            .replace('\r', "%0D")
            .replace('\n', "%0A")
            .replace(':', "%3A")
            .replace(',', "%2C");
        println!(
            "::{level} file={},line={},title={}::[{}] {message}",
            file,
            finding.line,
            finding.pattern_id,
            confidence(finding.confidence)
        );
    }
}

pub(crate) fn write(root: &Path, path: &Path, content: &str) -> Result<(), String> {
    let target = root.join(path);
    fs::write(&target, content).map_err(|error| format!("{}: {error}", target.display()))?;
    Ok(())
}
