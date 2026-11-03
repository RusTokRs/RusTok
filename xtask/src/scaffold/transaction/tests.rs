use super::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::Duration;

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustok-recovery-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir_all(path.join("crates/modules")).unwrap();
        fs::write(path.join("modules.toml"), "before").unwrap();
        Self(path)
    }

    fn plan(&self) -> Vec<PlannedFile> {
        plan(&self.0)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("Test root cleanup failed: {error}");
        }
    }
}

fn plan(root: &Path) -> Vec<PlannedFile> {
    vec![
        PlannedFile {
            path: root.join("crates/modules/rustok-example/src/lib.rs"),
            content: "pub struct Example;".into(),
            original: None,
        },
        PlannedFile {
            path: root.join("crates/modules/rustok-example/README.md"),
            content: "Example module".into(),
            original: None,
        },
        PlannedFile {
            path: root.join("modules.toml"),
            content: "after".into(),
            original: Some(b"before".to_vec()),
        },
    ]
}

pub(super) fn checkpoint(name: &str) {
    if std::env::var("RUSTOK_SCAFFOLD_TEST_PAUSE").as_deref() == Ok(name) {
        println!("SCAFFOLD_CHECKPOINT {name}");
        std::io::stdout().flush().unwrap();
        loop {
            std::thread::park();
        }
    }
}

fn worker_command(root: &Path, mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "scaffold::transaction::tests::process_worker",
            "--ignored",
            "--nocapture",
        ])
        .env("RUSTOK_SCAFFOLD_TEST_ROOT", root)
        .env("RUSTOK_SCAFFOLD_TEST_MODE", mode)
        .env_remove("RUSTOK_SCAFFOLD_TEST_PAUSE")
        .stdin(Stdio::null());
    command
}

struct PausedChild(Option<Child>);

impl PausedChild {
    fn start(root: &Path, mode: &str, pause: &str) -> Self {
        let mut child = worker_command(root, mode)
            .env("RUSTOK_SCAFFOLD_TEST_PAUSE", pause)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if send.send(line).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        eprintln!("Worker stdout failed: {error}");
                        break;
                    }
                }
            }
        });
        let guard = Self(Some(child));
        loop {
            let line = receive
                .recv_timeout(Duration::from_secs(30))
                .unwrap_or_else(|error| panic!("Worker did not reach {pause}: {error}"));
            if line == format!("SCAFFOLD_CHECKPOINT {pause}") {
                break;
            }
        }
        guard
    }

    fn kill(mut self) {
        let mut child = self.0.take().unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
    }
}

impl Drop for PausedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => {
                    if let Err(error) = child.kill() {
                        eprintln!("Cannot kill test worker: {error}");
                    }
                    if let Err(error) = child.wait() {
                        eprintln!("Cannot reap test worker: {error}");
                    }
                }
                Err(error) => eprintln!("Cannot inspect test worker: {error}"),
            }
        }
    }
}

fn run_worker(root: &Path, mode: &str) -> std::process::Output {
    worker_command(root, mode).output().unwrap()
}

fn assert_recovered(root: &TestRoot) {
    let output = run_worker(&root.0, "recover");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(root.0.join("modules.toml")).unwrap(), b"before");
    assert!(!root.0.join("crates/modules/rustok-example").exists());
    assert!(!root.0.join(".rustok/scaffold/active").exists());
    assert!(!root.0.join(".rustok/scaffold/retired").exists());
}

#[test]
#[ignore = "Subprocess harness entrypoint; requires a parent-owned temporary workspace"]
fn process_worker() {
    let root = PathBuf::from(std::env::var_os("RUSTOK_SCAFFOLD_TEST_ROOT").expect("worker root"));
    let mode = std::env::var("RUSTOK_SCAFFOLD_TEST_MODE").expect("worker mode");
    if matches!(mode.as_str(), "orchestrate" | "contender") {
        let mut options = super::super::tests::options();
        options.dry_run = false;
        options.skip_validate = true;
        if mode == "contender" {
            options.slug = "contender".into();
        }
        let error = super::super::scaffold_in_workspace(&root, &options, |_, _| {
            checkpoint("validation");
            bail!("injected validation failure")
        })
        .unwrap_err();
        let expected = if mode == "contender" {
            "workspace is busy"
        } else {
            "injected validation failure"
        };
        assert!(error.to_string().contains(expected), "{error:#}");
        return;
    }
    let workspace = ScaffoldWorkspace::acquire(&root).unwrap();
    if mode == "acquire" {
        return;
    }
    workspace.recover().unwrap();
    if mode == "recover" {
        return;
    }
    let transaction = if mode == "partial" {
        workspace
            .apply_with_writer(
                &plan(&root),
                &root.join("crates/modules/rustok-example"),
                |file, bytes| file.write(&bytes[..bytes.len().min(3)]),
            )
            .unwrap()
    } else {
        workspace
            .apply(&plan(&root), &root.join("crates/modules/rustok-example"))
            .unwrap()
    };
    transaction.commit().unwrap();
}

#[test]
fn different_slugs_share_one_process_lock_and_killed_owner_releases_it() {
    let root = TestRoot::new();
    let other_root = TestRoot::new();
    let owner = PausedChild::start(&root.0, "run", "prepared");
    let contender = run_worker(&root.0, "contender");
    assert!(
        contender.status.success(),
        "{}",
        String::from_utf8_lossy(&contender.stderr)
    );
    assert!(run_worker(&other_root.0, "acquire").status.success());
    owner.kill();
    assert!(run_worker(&root.0, "acquire").status.success());
    assert_recovered(&root);
    assert!(root.0.join(".rustok/scaffold/lock").is_file());
}

#[test]
fn interrupted_apply_recovers_in_a_fresh_process_at_each_boundary() {
    for point in [
        "prepared",
        "claimed",
        "source",
        "registration",
        "before_commit",
    ] {
        let root = TestRoot::new();
        PausedChild::start(&root.0, "run", point).kill();
        assert_recovered(&root);
        assert_recovered(&root);
    }
}

#[test]
fn partial_staging_is_never_published_and_restart_cleans_it() {
    let root = TestRoot::new();
    PausedChild::start(&root.0, "partial", "staging_write").kill();
    assert_eq!(fs::read(root.0.join("modules.toml")).unwrap(), b"before");
    assert!(!root.0.join("crates/modules/rustok-example").exists());
    assert_recovered(&root);
    assert!(!root.0.join(".rustok/scaffold/preparing").exists());
}

#[test]
fn interrupted_commit_and_retirement_never_undo_committed_sources() {
    for point in ["committed", "before_retire", "retired"] {
        let root = TestRoot::new();
        PausedChild::start(&root.0, "run", point).kill();
        let output = run_worker(&root.0, "recover");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(root.0.join("modules.toml")).unwrap(), b"after");
        assert_eq!(
            fs::read_to_string(root.0.join("crates/modules/rustok-example/src/lib.rs")).unwrap(),
            "pub struct Example;"
        );
        assert!(!root.0.join(".rustok/scaffold/active").exists());
        assert!(!root.0.join(".rustok/scaffold/retired").exists());
    }
}

#[test]
fn failed_commit_marker_creation_rolls_back_before_returning() {
    let root = TestRoot::new();
    let transaction = ScaffoldTransaction::apply(
        &root.0,
        &root.plan(),
        &root.0.join("crates/modules/rustok-example"),
    )
    .unwrap();
    fs::write(
        root.0.join(".rustok/scaffold/active/decision.pending"),
        "partial",
    )
    .unwrap();
    let error = transaction.commit().unwrap_err();
    assert!(error.to_string().contains("recovery completed"));
    assert_eq!(fs::read(root.0.join("modules.toml")).unwrap(), b"before");
    assert!(!root.0.join("crates/modules/rustok-example").exists());
    assert!(!root.0.join(".rustok/scaffold/active").exists());
}

#[test]
fn failed_retirement_cleanup_preserves_committed_sources() {
    let root = TestRoot::new();
    let transaction = ScaffoldTransaction::apply(
        &root.0,
        &root.plan(),
        &root.0.join("crates/modules/rustok-example"),
    )
    .unwrap();
    fs::write(
        root.0.join(".rustok/scaffold/active/foreign.txt"),
        "preserve",
    )
    .unwrap();
    assert!(transaction
        .commit()
        .unwrap_err()
        .to_string()
        .contains("Scaffold committed"));
    assert!(!run_worker(&root.0, "recover").status.success());
    let foreign = root.0.join(".rustok/scaffold/retired/foreign.txt");
    assert_eq!(fs::read(&foreign).unwrap(), b"preserve");
    fs::remove_file(foreign).unwrap();
    assert!(run_worker(&root.0, "recover").status.success());
    assert_eq!(fs::read(root.0.join("modules.toml")).unwrap(), b"after");
    assert!(root
        .0
        .join("crates/modules/rustok-example/src/lib.rs")
        .is_file());
}

#[test]
fn interrupted_recovery_repeats_from_already_restored_files() {
    let root = TestRoot::new();
    PausedChild::start(&root.0, "run", "registration").kill();
    PausedChild::start(&root.0, "recover", "restored").kill();
    assert_recovered(&root);
}

#[test]
fn ambiguous_directory_claim_requires_reconciliation() {
    let root = TestRoot::new();
    PausedChild::start(&root.0, "run", "unclaimed").kill();
    assert!(!run_worker(&root.0, "recover").status.success());
    assert!(root.0.join(".rustok/scaffold/active/plan.json").is_file());
    assert!(root.0.join("crates/modules/rustok-example").is_dir());
    fs::remove_dir(root.0.join("crates/modules/rustok-example")).unwrap();
    assert_recovered(&root);
}

#[test]
fn lock_survives_validation_and_interrupted_rollback() {
    let root = TestRoot::new();
    super::super::tests::copy_scaffold_inputs(&root.0);
    let before = fs::read(root.0.join("modules.toml")).unwrap();
    let owner = PausedChild::start(&root.0, "orchestrate", "validation");
    assert!(!run_worker(&root.0, "acquire").status.success());
    owner.kill();
    let recovering = PausedChild::start(&root.0, "recover", "restored");
    assert!(!run_worker(&root.0, "acquire").status.success());
    recovering.kill();
    let output = run_worker(&root.0, "recover");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(root.0.join("modules.toml")).unwrap(), before);
    assert!(!root.0.join("crates/modules/rustok-example").exists());
}

#[test]
fn external_changes_and_untracked_files_block_undo_without_deleting_anything() {
    let root = TestRoot::new();
    PausedChild::start(&root.0, "run", "registration").kill();
    let owner_file = root.0.join("crates/modules/rustok-example/owner.txt");
    fs::write(&owner_file, "owner work").unwrap();
    fs::write(root.0.join("modules.toml"), "external edit").unwrap();
    assert!(!run_worker(&root.0, "recover").status.success());
    assert_eq!(
        fs::read(root.0.join("modules.toml")).unwrap(),
        b"external edit"
    );
    assert_eq!(fs::read(&owner_file).unwrap(), b"owner work");
    assert!(root
        .0
        .join("crates/modules/rustok-example/src/lib.rs")
        .is_file());
    assert!(root.0.join(".rustok/scaffold/active/plan.json").is_file());
    fs::remove_file(owner_file).unwrap();
    assert!(!run_worker(&root.0, "recover").status.success());
    assert_eq!(
        fs::read(root.0.join("modules.toml")).unwrap(),
        b"external edit"
    );
    fs::write(root.0.join("modules.toml"), "after").unwrap();
    assert_recovered(&root);
}

#[test]
fn malformed_or_unsafe_journals_fail_before_recovery_writes() {
    for kind in [
        "json",
        "traversal",
        "absolute",
        "unrelated",
        "duplicate",
        "case-alias",
        "stream",
        "version",
        "workspace",
        "backup",
        "commit",
    ] {
        let root = TestRoot::new();
        let transaction = ScaffoldTransaction::apply(
            &root.0,
            &root.plan(),
            &root.0.join("crates/modules/rustok-example"),
        )
        .unwrap();
        drop(transaction);
        let path = root.0.join(".rustok/scaffold/active/plan.json");
        let mut journal: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match kind {
            "json" => fs::write(&path, "{").unwrap(),
            "backup" => {
                fs::write(root.0.join(".rustok/scaffold/active/backup/2"), "corrupt").unwrap()
            }
            "commit" => {
                fs::write(root.0.join(".rustok/scaffold/active/committed"), "corrupt").unwrap()
            }
            _ => {
                match kind {
                    "traversal" => journal["files"][0]["path"] = "../outside".into(),
                    "absolute" => {
                        journal["files"][0]["path"] =
                            root.0.join("outside").to_str().unwrap().into()
                    }
                    "unrelated" => journal["files"][2]["path"] = "unrelated.txt".into(),
                    "duplicate" => {
                        journal["files"][1]["path"] = journal["files"][0]["path"].clone()
                    }
                    "case-alias" => {
                        journal["files"][1]["path"] =
                            "crates/modules/rustok-example/src/LIB.rs".into()
                    }
                    "stream" => {
                        journal["files"][0]["path"] =
                            "crates/modules/rustok-example/src/lib.rs:extra".into()
                    }
                    "version" => journal["version"] = 99.into(),
                    "workspace" => {
                        journal["workspace"] = root.0.join("other").to_str().unwrap().into()
                    }
                    _ => unreachable!(),
                }
                fs::write(&path, serde_json::to_vec(&journal).unwrap()).unwrap();
            }
        }
        assert!(
            !run_worker(&root.0, "recover").status.success(),
            "accepted {kind}"
        );
        assert_eq!(fs::read(root.0.join("modules.toml")).unwrap(), b"after");
        assert!(root
            .0
            .join("crates/modules/rustok-example/src/lib.rs")
            .is_file());
        assert!(path.is_file());
    }
}

#[test]
fn preview_creates_no_state_and_never_repairs_pending_recovery() {
    let root = TestRoot::new();
    assert!(ScaffoldWorkspace::preview(&root.0).unwrap().is_none());
    assert!(!root.0.join(".rustok").exists());
    let transaction = ScaffoldTransaction::apply(
        &root.0,
        &root.plan(),
        &root.0.join("crates/modules/rustok-example"),
    )
    .unwrap();
    drop(transaction);
    let before = fs::read(root.0.join(".rustok/scaffold/active/plan.json")).unwrap();
    assert!(ScaffoldWorkspace::preview(&root.0).is_err());
    assert_eq!(
        fs::read(root.0.join(".rustok/scaffold/active/plan.json")).unwrap(),
        before
    );
    assert_eq!(fs::read(root.0.join("modules.toml")).unwrap(), b"after");
}

#[test]
fn read_only_registration_is_rejected_before_any_module_write() {
    let root = TestRoot::new();
    let manifest = root.0.join("modules.toml");
    let original_permissions = fs::metadata(&manifest).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&manifest, permissions).unwrap();
    let result = ScaffoldTransaction::apply(
        &root.0,
        &root.plan(),
        &root.0.join("crates/modules/rustok-example"),
    );
    fs::set_permissions(&manifest, original_permissions).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(manifest).unwrap(), b"before");
    assert!(!root.0.join("crates/modules/rustok-example").exists());
}

#[cfg(any(unix, windows))]
#[test]
fn redirected_module_ancestor_blocks_apply_and_recovery() {
    let root = TestRoot::new();
    let outside = TestRoot::new();
    let ancestor = root.0.join("crates").join("modules");
    let saved = root.0.join("crates").join("saved-modules");
    let destination = outside.0.join("crates").join("modules");
    let module = ancestor.join("rustok-example");
    for recovering in [false, true] {
        if recovering {
            PausedChild::start(&root.0, "run", "registration").kill();
        }
        fs::rename(&ancestor, &saved).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&destination, &ancestor).unwrap();
        #[cfg(windows)]
        {
            let result = Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&ancestor)
                .arg(&destination)
                .output()
                .unwrap();
            assert!(result.status.success(), "{:?}", result);
        }
        let error = if recovering {
            ScaffoldWorkspace::acquire(&root.0)
                .unwrap()
                .recover()
                .unwrap_err()
        } else {
            ScaffoldTransaction::apply(&root.0, &root.plan(), &module)
                .err()
                .unwrap()
        };
        #[cfg(unix)]
        fs::remove_file(&ancestor).unwrap();
        #[cfg(windows)]
        fs::remove_dir(&ancestor).unwrap();
        fs::rename(&saved, &ancestor).unwrap();
        assert!(
            error.to_string().contains("traverses a symlink"),
            "{error:#}"
        );
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        assert_eq!(
            fs::read(root.0.join("modules.toml")).unwrap(),
            if recovering {
                b"after".as_slice()
            } else {
                b"before".as_slice()
            }
        );
    }
    assert_recovered(&root);
}

#[cfg(windows)]
#[test]
fn windows_sharing_violation_preserves_recovery_until_target_is_released() {
    use std::os::windows::fs::OpenOptionsExt;
    const SHARE_READ_AND_WRITE: u32 = 0x1 | 0x2;

    let root = TestRoot::new();
    let manifest = root.0.join("modules.toml");
    let lock_target = || {
        OpenOptions::new()
            .read(true)
            .share_mode(SHARE_READ_AND_WRITE)
            .open(&manifest)
            .unwrap()
    };
    let module = root.0.join("crates/modules/rustok-example");

    let open = lock_target();
    assert!(ScaffoldTransaction::apply(&root.0, &root.plan(), &module).is_err());
    assert_eq!(fs::read(&manifest).unwrap(), b"before");
    assert!(!module.exists());
    drop(open);

    PausedChild::start(&root.0, "run", "registration").kill();
    let open = lock_target();
    assert!(!run_worker(&root.0, "recover").status.success());
    assert_eq!(fs::read(&manifest).unwrap(), b"after");
    assert!(root.0.join(".rustok/scaffold/active/backup/2").is_file());
    assert!(module.join("src/lib.rs").is_file());
    drop(open);
    assert_recovered(&root);
}
