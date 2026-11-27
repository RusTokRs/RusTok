use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn command(root: &Path, executable: &str, args: &[&str]) -> Output {
    Command::new(executable)
        .current_dir(root)
        .args(args)
        .env(
            "GIT_CEILING_DIRECTORIES",
            root.parent().expect("test directory has a parent"),
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .expect("command executes")
}

fn git(root: &Path, args: &[&str]) {
    let output = command(root, "git", args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn doctor(root: &Path, args: &[&str]) -> Output {
    command(root, env!("CARGO_BIN_EXE_rustok-doctor"), args)
}

fn compiler_message(code: &str, path: &Path, line: usize, column: usize, text: &str) -> Value {
    serde_json::json!({
        "reason": "compiler-message",
        "message": {
            "code": {"code": code},
            "level": "warning",
            "spans": [{
                "file_name": path,
                "line_start": line,
                "line_end": line,
                "column_start": column,
                "is_primary": true,
                "text": [{"text": text}]
            }]
        }
    })
}

fn write_cargo_capture(path: &Path, messages: &[Value], success: Option<bool>) {
    let mut lines: Vec<_> = messages.iter().map(Value::to_string).collect();
    if let Some(success) = success {
        lines.push(serde_json::json!({"reason":"build-finished", "success":success}).to_string());
    }
    fs::write(path, lines.join("\n")).unwrap();
}

#[test]
fn cargo_diagnostics_join_selected_sources_and_existing_reports() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let selected = Path::new("selected.rs");
    let line = "pub fn check() {}";
    // CRLF working-tree source and the normalized text in Cargo spans.
    fs::write(root.join(selected), format!("{line}\r\n")).unwrap();
    fs::write(root.join("other.rs"), line).unwrap();
    let mappings = [
        (
            "let_underscore_lock",
            "P-RUST-LET-UNDERSCORE-LOCK",
            "rustc",
            "high",
        ),
        (
            "clippy::let_underscore_future",
            "P-RUST-LET-UNDERSCORE-FUTURE",
            "clippy",
            "high",
        ),
        (
            "clippy::unused_io_amount",
            "P-RUST-UNUSED-IO-AMOUNT",
            "clippy",
            "high",
        ),
        (
            "static_mut_refs",
            "P-RUST-STATIC-MUT-REFERENCE",
            "rustc",
            "high",
        ),
        (
            "clippy::await_holding_refcell_ref",
            "P-RUST-AWAIT-HOLDING-REFCELL",
            "clippy",
            "heuristic",
        ),
        (
            "clippy::arc_with_non_send_sync",
            "P-RUST-ARC-NONSEND-SYNC",
            "clippy",
            "heuristic",
        ),
    ];
    let mut messages = vec![serde_json::json!({"reason":"compiler-artifact", "fresh":true})];
    messages.extend(
        mappings
            .iter()
            .map(|(code, ..)| compiler_message(code, selected, 1, 1, line)),
    );
    // Cargo can replay diagnostics for multiple compilation targets.
    messages.push(compiler_message(
        mappings[0].0,
        &root.join(selected),
        1,
        1,
        line,
    ));
    messages.push(compiler_message(
        mappings[0].0,
        Path::new("other.rs"),
        1,
        1,
        line,
    ));
    messages.push(compiler_message("clippy::unmapped", selected, 1, 1, line));
    let mut secondary = compiler_message(mappings[0].0, selected, 200, 1, line);
    secondary["message"]["spans"][0]["is_primary"] = Value::Bool(false);
    messages.push(secondary);
    write_cargo_capture(&root.join("cargo.jsonl"), &messages, Some(true));

    let output = doctor(
        root,
        &[
            "hygiene",
            "--path",
            "selected.rs",
            "--cargo-diagnostics",
            "cargo.jsonl",
            "--fail-on",
            "P0",
            "--json",
            "--csv",
            "out.csv",
            "--markdown-report",
            "out.md",
            "--patch",
            "out.patch",
        ],
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["cargo_diagnostics"]["build_succeeded"], true);
    let findings = report["findings"].as_array().unwrap();
    assert_eq!(findings.len(), mappings.len());
    let mut csv = csv::Reader::from_path(root.join("out.csv")).unwrap();
    assert_eq!(csv.records().count(), findings.len());
    let markdown = fs::read_to_string(root.join("out.md")).unwrap();
    let patch = fs::read_to_string(root.join("out.patch")).unwrap();
    for (code, id, kind, confidence) in mappings {
        let finding = findings
            .iter()
            .find(|finding| finding["pattern_id"] == id)
            .unwrap();
        assert_eq!(finding["file"], "selected.rs");
        assert_eq!(finding["content"], line);
        assert_eq!(finding["confidence"], confidence);
        assert_eq!(
            finding["evidence"],
            serde_json::json!({"kind": kind, "code": code, "column": 1})
        );
        assert!(markdown.contains(code));
        assert!(patch.contains(id));
    }
    let advisory_only = doctor(
        root,
        &[
            "hygiene",
            "--path",
            "selected.rs",
            "--cargo-diagnostics",
            "cargo.jsonl",
            "--strict",
            "--ignore-rule",
            "P-RUST-LET-UNDERSCORE-LOCK,P-RUST-LET-UNDERSCORE-FUTURE,P-RUST-UNUSED-IO-AMOUNT,P-RUST-STATIC-MUT-REFERENCE",
        ],
    );
    assert!(advisory_only.status.success(), "{advisory_only:?}");
}

#[test]
fn cargo_diagnostics_fail_closed_and_keep_findings_from_failed_builds() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let source = Path::new("selected.rs");
    let line = "pub fn check() {}";
    fs::write(root.join(source), line).unwrap();
    let capture = root.join("cargo.jsonl");
    let message = compiler_message("let_underscore_lock", source, 1, 1, line);
    let args = [
        "hygiene",
        "--path",
        "selected.rs",
        "--cargo-diagnostics",
        "cargo.jsonl",
        "--json",
    ];
    write_cargo_capture(&capture, std::slice::from_ref(&message), Some(false));
    let output = doctor(root, &args);
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["cargo_diagnostics"]["build_succeeded"], false);
    assert_eq!(report["total_findings"], 1);

    write_cargo_capture(&capture, std::slice::from_ref(&message), None);
    let incomplete = fs::read_to_string(&capture).unwrap();
    let finished = serde_json::json!({"reason":"build-finished", "success":true});
    for (input, expected_error) in [
        (incomplete, "missing Cargo build-finished"),
        ("not json".to_owned(), "cargo.jsonl:1"),
        (format!("{finished}\n{message}"), "unexpected record after"),
    ] {
        fs::write(&capture, input).unwrap();
        let output = doctor(root, &args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected_error));
    }

    write_cargo_capture(&capture, &[message], Some(true));
    let original = fs::read(&capture).unwrap();
    let mut overwrite = args.to_vec();
    overwrite.extend(["--csv", "cargo.jsonl"]);
    assert_eq!(doctor(root, &overwrite).status.code(), Some(2));
    assert_eq!(fs::read(&capture).unwrap(), original);

    fs::write(root.join(source), "pub fn changed() {}").unwrap();
    let stale = doctor(root, &args);
    assert_eq!(stale.status.code(), Some(2));
    assert!(stale.stdout.is_empty());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("does not match selected source"));

    let staged = doctor(
        root,
        &["hygiene", "--staged", "--cargo-diagnostics", "cargo.jsonl"],
    );
    assert_eq!(staged.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&staged.stderr).contains("working-tree evidence"));
}

#[test]
fn conflicting_targets_and_failure_thresholds_are_rejected() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let cases: &[&[&str]] = &[
        &["hygiene", "--changed", "--path", "missing.rs", "--json"],
        &["hygiene", "--crate", "product", "--all-modules"],
        &["hygiene", "--staged", "--diff", "HEAD"],
        &["hygiene", "--all", "--all-modules"],
        &["hotspots", "--path", ".", "--all"],
        &["hygiene", "--strict", "--fail-on", "P0"],
    ];
    for args in cases {
        let output = doctor(root, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("mutually exclusive"),
            "{args:?}"
        );
    }
}

#[test]
fn markdown_and_csv_include_every_json_finding() {
    let dir = TempDir::new().unwrap();
    let source = (0..25)
        .map(|index| format!("fn example_{index}(v: Option<u8>) {{ v.unwrap(); }}\n"))
        .collect::<String>();
    fs::write(dir.path().join("source.rs"), source).unwrap();
    let output = doctor(
        dir.path(),
        &[
            "--path",
            "source.rs",
            "--json",
            "--csv",
            "all.csv",
            "--markdown-report",
            "all.md",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let findings = json["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 25);
    assert_eq!(findings[0]["evidence"]["kind"], "lexical");
    let mut csv = csv::Reader::from_path(dir.path().join("all.csv")).unwrap();
    assert_eq!(csv.records().count(), findings.len());
    let markdown = fs::read_to_string(dir.path().join("all.md")).unwrap();
    for finding in findings {
        let line = finding["line"].as_u64().unwrap();
        assert!(
            markdown.contains(&format!("`source.rs:{line}`")),
            "missing line {line}"
        );
    }
    assert!(!markdown.contains("more occurrences"));
}

#[test]
fn staged_scan_uses_index_blobs_and_patches_them() {
    let dir = TempDir::new().expect("temporary Git repository");
    let root = dir.path();
    git(root, &["init", "-q", "--template="]);
    let bad = b"mod other;\ninclude!(\"handwritten.rs\");\n";
    let clean = b"fn run() {}\n";
    fs::write(root.join("staged bad.rs"), bad).unwrap();
    fs::write(root.join("staged clean.rs"), clean).unwrap();
    fs::write(root.join("missing.rs"), bad).unwrap();
    git(
        root,
        &[
            "add",
            "--",
            "staged bad.rs",
            "staged clean.rs",
            "missing.rs",
        ],
    );
    fs::write(root.join("staged bad.rs"), clean).unwrap();
    fs::write(root.join("staged clean.rs"), bad).unwrap();
    fs::remove_file(root.join("missing.rs")).unwrap();

    let output = doctor(root, &["--staged", "--strict", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["scanned_files"], 3);
    assert_eq!(report["findings"].as_array().unwrap().len(), 2);
    assert_eq!(report["findings"][0]["file"], "missing.rs");
    assert_eq!(report["findings"][1]["file"], "staged bad.rs");

    let output = doctor(root, &["--staged", "--patch", "staged.patch"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    git(root, &["apply", "--cached", "--check", "staged.patch"]);
}

#[test]
fn invalid_ref_and_thresholds_preserve_exit_codes() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "--template="]);
    let output = doctor(root, &["--diff", "missing-test-ref", "--strict", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing-test-ref"));

    fs::write(root.join("service.rs"), b"include!(\"handwritten.rs\");\nfn allowed(scope: PermissionScope) -> bool {\n    match scope {\n        PermissionScope::None => true,\n        _ => false,\n    }\n}\n").unwrap();
    for (level, expected) in [("P0", 0), ("P1", 1), ("P2", 1)] {
        for json in [false, true] {
            let mut args = vec!["--path", "service.rs", "--fail-on", level];
            if json {
                args.push("--json");
            }
            let output = doctor(root, &args);
            assert_eq!(
                output.status.code(),
                Some(expected),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if json {
                let report: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(report["findings"][0]["pattern_id"], "P-ZERO-INCLUDE");
                assert_eq!(report["findings"][1]["pattern_id"], "P-FAIL-CLOSED");
            }
        }
    }
    let output = doctor(
        root,
        &[
            "--path",
            "service.rs",
            "--strict",
            "--ignore-rule",
            "P-ZERO-INCLUDE",
            "--github-annotations",
        ],
    );
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("::warning file=service.rs,line=4,title=P-FAIL-CLOSED::[heuristic]")
    );
}

#[test]
fn patch_preserves_crlf_and_missing_final_newline() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "--template="]);
    let original = b"fn run() {\r\n    let _ = dangerous_operation().unwrap();\r\n}";
    fs::write(root.join("source file.rs"), original).unwrap();
    let output = doctor(root, &["--path", "source file.rs", "--patch", "out.patch"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    git(root, &["apply", "--check", "out.patch"]);
    git(root, &["apply", "out.patch"]);
    let patched = fs::read(root.join("source file.rs")).unwrap();
    assert!(patched.starts_with(original));
    assert!(String::from_utf8_lossy(&patched).contains("// REMEDIATION (P-ANTI-SWALLOW, line 2):"));
    assert!(
        String::from_utf8_lossy(&patched).contains("// REMEDIATION (P-ZERO-PANIC-PROD, line 2):")
    );
}

#[test]
fn changed_and_diff_select_complete_working_tree_files() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "--template="]);
    fs::write(root.join("tracked.rs"), b"fn original() {}\n").unwrap();
    git(root, &["add", "--", "tracked.rs"]);
    git(
        root,
        &[
            "-c",
            "user.name=Doctor Test",
            "-c",
            "user.email=doctor@example.invalid",
            "commit",
            "-qm",
            "baseline",
        ],
    );
    fs::write(root.join("tracked.rs"), b"include!(\"tracked.rs\");\n").unwrap();
    fs::write(root.join("untracked.rs"), b"include!(\"untracked.rs\");\n").unwrap();

    let changed = doctor(root, &["--changed", "--json"]);
    assert!(
        changed.status.success(),
        "{}",
        String::from_utf8_lossy(&changed.stderr)
    );
    let changed: Value = serde_json::from_slice(&changed.stdout).unwrap();
    assert_eq!(changed["scanned_files"], 2);
    assert_eq!(changed["findings"][0]["file"], "tracked.rs");
    assert_eq!(changed["findings"][1]["file"], "untracked.rs");

    let diff = doctor(root, &["--diff", "HEAD", "--json"]);
    assert!(
        diff.status.success(),
        "{}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let diff: Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert_eq!(diff["scanned_files"], 1);
    assert_eq!(diff["findings"][0]["file"], "tracked.rs");
}

#[test]
fn parallel_scan_keeps_findings_in_file_order() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    for number in 0..24 {
        fs::write(
            root.join(format!("file-{number:02}.rs")),
            b"include!(\"child.rs\");\n",
        )
        .unwrap();
    }
    let serial = doctor(root, &["--path", ".", "--threads", "1", "--json"]);
    let parallel = doctor(root, &["--path", ".", "--threads", "4", "--json"]);
    assert!(
        serial.status.success(),
        "{}",
        String::from_utf8_lossy(&serial.stderr)
    );
    assert!(
        parallel.status.success(),
        "{}",
        String::from_utf8_lossy(&parallel.stderr)
    );
    let serial: Value = serde_json::from_slice(&serial.stdout).unwrap();
    let parallel: Value = serde_json::from_slice(&parallel.stdout).unwrap();
    assert_eq!(serial, parallel);
}

#[test]
fn crate_resolution_and_reports_use_the_same_findings() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let source = root.join("crates/modules/rustok-forum/src");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("service.rs"), b"include!(\"local.rs\");\n").unwrap();
    let output = doctor(
        root,
        &[
            "--crate",
            "forum",
            "--markdown-report",
            "report.md",
            "--csv",
            "report.csv",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let markdown = fs::read_to_string(root.join("report.md")).unwrap();
    let csv = fs::read_to_string(root.join("report.csv")).unwrap();
    assert!(markdown.contains("P-ZERO-INCLUDE"));
    assert!(markdown.contains("`rustok-forum`"));
    assert!(csv.contains("P-ZERO-INCLUDE"));
    assert!(csv.contains("crates/modules/rustok-forum/src/service.rs"));
    assert!(csv.contains("high"));
}

#[test]
fn requested_reports_are_written_for_json_and_clean_scans() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("source.rs"), b"include!(\"local.rs\");\n").unwrap();
    let output = doctor(
        root,
        &[
            "--path",
            "source.rs",
            "--json",
            "--csv",
            "report.csv",
            "--markdown-report",
            "report.md",
            "--patch",
            "report.patch",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["total_findings"], 1);
    assert!(
        fs::read_to_string(root.join("report.csv"))
            .unwrap()
            .contains("P-ZERO-INCLUDE")
    );
    assert!(
        fs::read_to_string(root.join("report.md"))
            .unwrap()
            .contains("P-ZERO-INCLUDE")
    );
    assert!(
        fs::read_to_string(root.join("report.patch"))
            .unwrap()
            .contains("P-ZERO-INCLUDE")
    );

    fs::write(root.join("source.rs"), b"fn clean() {}\n").unwrap();
    let output = doctor(
        root,
        &[
            "--path",
            "source.rs",
            "--json",
            "--csv",
            "report.csv",
            "--markdown-report",
            "report.md",
            "--patch",
            "report.patch",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["total_findings"], 0);
    assert!(
        !fs::read_to_string(root.join("report.csv"))
            .unwrap()
            .contains("P-ZERO-INCLUDE")
    );
    assert!(
        !fs::read_to_string(root.join("report.md"))
            .unwrap()
            .contains("P-ZERO-INCLUDE")
    );
    assert!(fs::read(root.join("report.patch")).unwrap().is_empty());

    let output = doctor(
        root,
        &[
            "--path",
            "source.rs",
            "--json",
            "--csv",
            "missing/report.csv",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let output = doctor(root, &["--json", "--github-annotations"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn output_paths_cannot_overwrite_sources_or_each_other() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let original = b"include!(\"local.rs\");\n";
    fs::write(root.join("source.rs"), original).unwrap();
    let output = doctor(root, &["--path", "source.rs", "--csv", "source.rs"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(fs::read(root.join("source.rs")).unwrap(), original);
    let output = doctor(
        root,
        &["--path", "source.rs", "--csv", "same", "--patch", "same"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!root.join("same").exists());
    let output = doctor(root, &["--crate", "missing,also_missing", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    fs::create_dir_all(root.join("crates/modules/rustok-forum")).unwrap();
    fs::create_dir_all(root.join("crates/modules/rustok-forum-extra")).unwrap();
    let output = doctor(root, &["--crate", "forum,missing", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    let output = doctor(root, &["--crate", "foru", "--json"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn annotations_escape_file_properties() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("source,part.rs"),
        b"include!(\"local.rs\");\n",
    )
    .unwrap();
    let output = doctor(
        dir.path(),
        &["--path", "source,part.rs", "--github-annotations"],
    );
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("file=source%2Cpart.rs,"));
}

#[test]
fn strict_mode_rejects_multiline_include() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("source.rs"),
        b"include!(\n \"local.rs\"\n);\n",
    )
    .unwrap();
    let output = doctor(dir.path(), &["--path", "source.rs", "--strict", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["findings"][0]["pattern_id"], "P-ZERO-INCLUDE");
    assert_eq!(report["findings"][0]["line"], 1);
}

#[test]
fn hotspots_rank_selected_files_and_exclude_generated_and_vendored() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::create_dir_all(root.join("vendor")).unwrap();
    fs::write(root.join("src/long.rs"), "a\nb\nc\n").unwrap();
    fs::write(root.join("src/short.rs"), "a\n").unwrap();
    fs::write(root.join("tests/test.rs"), "a\nb\n").unwrap();
    fs::write(root.join("src/generated.rs"), "// @generated\na\nb\nc\nd\n").unwrap();
    fs::write(root.join("vendor/copied.rs"), "a\nb\nc\nd\n").unwrap();
    let output = doctor(root, &["hotspots", "--path", ".", "--top", "2", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selected_files"], 5);
    assert_eq!(report["ranked_files"], 3);
    assert_eq!(report["excluded_generated"], 1);
    assert_eq!(report["excluded_vendor"], 1);
    assert_eq!(report["files"][0]["file"], "src/long.rs");
    assert_eq!(report["files"][0]["lines"], 3);
    assert_eq!(report["files"][1]["kind"], "test");
    assert_eq!(
        doctor(root, &["hotspots", "--top", "0"]).status.code(),
        Some(2)
    );
}

#[test]
fn hotspots_measure_staged_contents() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "--template="]);
    fs::write(root.join("source.rs"), b"a\nb\nc\n").unwrap();
    git(root, &["add", "--", "source.rs"]);
    fs::write(root.join("source.rs"), b"a\n").unwrap();
    let output = doctor(root, &["hotspots", "--staged", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["files"][0]["lines"], 3);
}
