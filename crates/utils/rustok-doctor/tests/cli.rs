use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn command(root: &Path, executable: &str, args: &[&str]) -> Output {
    Command::new(executable)
        .current_dir(root)
        .args(args)
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
