use super::{PlannedFile, ensure_no_symlink_ancestors};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

const MAX_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 128;
const JOURNAL_VERSION: u32 = 1;
const RUNNER_NAME: &str = if cfg!(windows) {
    "recover.exe"
} else {
    "recover"
};

pub(super) struct ScaffoldWorkspace {
    root: PathBuf,
    state: PathBuf,
    // Closing this handle releases the OS lock. Its permanent inode is never removed.
    _lock: File,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u32,
    workspace: PathBuf,
    module_dir: PathBuf,
    files: Vec<JournalFile>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalFile {
    path: PathBuf,
    original: Option<String>,
    content: String,
}

pub(super) struct ScaffoldTransaction {
    workspace: ScaffoldWorkspace,
    journal: Journal,
    digest: String,
}

fn present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("Cannot inspect {}", path.display())),
    }
}

fn read_regular(root: &Path, path: &Path) -> Result<Option<Vec<u8>>> {
    ensure_no_symlink_ancestors(root, path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file(),
                "Expected regular file: {}",
                path.display()
            );
            ensure!(
                metadata.len() <= MAX_JOURNAL_BYTES,
                "Scaffold target exceeds its size limit: {}",
                path.display()
            );
            let mut bytes = Vec::new();
            File::open(path)?
                .take(MAX_JOURNAL_BYTES + 1)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() as u64 <= MAX_JOURNAL_BYTES,
                "Scaffold target exceeds its size limit: {}",
                path.display()
            );
            Ok(Some(bytes))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("Cannot inspect {}", path.display())),
    }
}

fn write_synced(path: &Path, content: &[u8]) -> Result<()> {
    let mut file = File::create_new(path)?;
    file.write_all(content)?;
    file.sync_all()?;
    Ok(())
}

fn sync_file(path: &Path) -> Result<()> {
    // FlushFileBuffers on Windows requires a handle with GENERIC_WRITE.
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?
        .sync_all()?;
    Ok(())
}

fn normal_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|part| {
            let Component::Normal(name) = part else {
                return false;
            };
            let Some(name) = name.to_str() else {
                return false;
            };
            // Exclude Win32 aliases, alternate data streams and short names on all platforms.
            let stem = name
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            !name.starts_with('.')
                && !name.ends_with('.')
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
                && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && stem.as_bytes()[3].is_ascii_digit())
        })
}

impl ScaffoldWorkspace {
    pub(super) fn recover_command(build_root: &Path) -> Result<()> {
        let executable = std::env::current_exe()?;
        let suffix = Path::new(".rustok/scaffold").join(RUNNER_NAME);
        let root = if executable.ends_with(suffix) {
            executable
                .ancestors()
                .nth(3)
                .context("Missing scaffold recovery workspace")?
        } else {
            build_root
        };
        Self::acquire(root)?.recover()
    }

    pub(super) fn acquire(root: &Path) -> Result<Self> {
        let state = root.join(".rustok/scaffold");
        for directory in [root.join(".rustok"), state.clone()] {
            ensure_no_symlink_ancestors(root, &directory)?;
            if !present(&directory)? {
                match fs::create_dir(&directory) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        return Err(error).context("Cannot create scaffold control directory");
                    }
                }
            }
            ensure_no_symlink_ancestors(root, &directory)?;
            ensure!(
                directory.is_dir(),
                "Scaffold control path is not a directory"
            );
        }
        Self::lock(root, state, false)
    }

    fn lock(root: &Path, state: PathBuf, preview: bool) -> Result<Self> {
        let path = state.join("lock");
        ensure_no_symlink_ancestors(root, &path)?;
        if present(&path)? {
            ensure!(
                fs::symlink_metadata(&path)?.is_file(),
                "Scaffold lock is not a regular file"
            );
        }
        let file = OpenOptions::new()
            .read(true)
            .write(!preview)
            .create(!preview)
            .truncate(false)
            .open(&path)?;
        let result = if preview {
            file.try_lock_shared()
        } else {
            file.try_lock()
        };
        match result {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => bail!(
                "Scaffold workspace is busy; another process holds {}. Retry after it finishes; do not delete the lock file",
                path.display()
            ),
            Err(fs::TryLockError::Error(error)) => {
                return Err(error).context("Filesystem cannot acquire the scaffold lock");
            }
        }
        Ok(Self {
            root: root.to_path_buf(),
            state,
            _lock: file,
        })
    }

    pub(super) fn preview(root: &Path) -> Result<Option<Self>> {
        let state = root.join(".rustok/scaffold");
        ensure_no_symlink_ancestors(root, &state)?;
        let guard = if present(&state.join("lock"))? {
            Some(Self::lock(root, state.clone(), true)?)
        } else {
            None
        };
        for name in ["preparing", "active", "retired"] {
            ensure!(
                !present(&state.join(name))?,
                "Scaffold recovery is pending at {}; dry-run never performs recovery. Run scaffold --recover",
                state.display()
            );
        }
        Ok(guard)
    }

    pub(super) fn retain_runner(&self) -> Result<()> {
        let runner = self.state.join(RUNNER_NAME);
        let executable = std::env::current_exe()?;
        ensure_no_symlink_ancestors(&self.root, &runner)?;
        if present(&runner)? && runner.canonicalize()? == executable.canonicalize()? {
            return Ok(());
        }
        let temporary = self.state.join("runner.pending");
        self.remove_control_file(&temporary)?;
        fs::copy(executable, &temporary)?;
        sync_file(&temporary)?;
        fs::rename(temporary, runner)
            .context("Cannot retain the standalone scaffold recovery CLI")?;
        Ok(())
    }

    pub(super) fn recover(&self) -> Result<()> {
        self.clean_private("preparing")?;
        self.clean_private("retired")?;
        let active = self.state.join("active");
        if !present(&active)? {
            return Ok(());
        }
        let (journal, digest) = self.read_journal()?;
        if let Some(marker) = read_regular(&self.root, &active.join("committed"))? {
            ensure!(
                marker == digest.as_bytes(),
                "Invalid scaffold commit marker; preserve {} for reconciliation",
                active.display()
            );
        } else {
            self.restore(&journal)?;
        }
        self.retire()
    }

    fn read_journal(&self) -> Result<(Journal, String)> {
        let bytes = read_regular(&self.root, &self.state.join("active/plan.json"))?
            .context("Missing scaffold recovery plan")?;
        let journal: Journal = serde_json::from_slice(&bytes)
            .context("Invalid scaffold recovery journal; preserve it for reconciliation")?;
        self.validate_journal(&journal)?;
        for (index, entry) in journal.files.iter().enumerate() {
            if let Some(original) = &entry.original {
                let backup = self.state.join(format!("active/backup/{index}"));
                ensure!(
                    read_regular(&self.root, &backup)?.as_deref() == Some(original.as_bytes()),
                    "Missing or changed scaffold backup: {}",
                    backup.display()
                );
            }
        }
        Ok((journal, hex::encode(Sha256::digest(bytes))))
    }

    fn validate_journal(&self, journal: &Journal) -> Result<()> {
        ensure!(
            journal.version == JOURNAL_VERSION,
            "Unsupported scaffold journal version: {}",
            journal.version
        );
        ensure!(
            journal.workspace == self.root.canonicalize()?,
            "Scaffold journal belongs to another workspace"
        );
        ensure!(
            normal_relative(&journal.module_dir),
            "Invalid scaffold module path"
        );
        let name = journal
            .module_dir
            .file_name()
            .and_then(|name| name.to_str())
            .context("Invalid module name")?;
        let slug = name
            .strip_prefix("rustok-")
            .context("Invalid scaffold crate name")?
            .replace('-', "_");
        ensure!(
            super::is_valid_slug(&slug)
                && name == format!("rustok-{}", slug.replace('_', "-"))
                && journal.module_dir == Path::new("crates/modules").join(name),
            "Invalid scaffold module identity"
        );
        ensure!(
            !journal.files.is_empty() && journal.files.len() <= MAX_FILES,
            "Invalid scaffold journal file count"
        );
        let mut seen = HashSet::new();
        for entry in &journal.files {
            ensure!(
                normal_relative(&entry.path)
                    && seen.insert(
                        entry
                            .path
                            .components()
                            .map(|part| part.as_os_str().to_string_lossy().to_ascii_lowercase())
                            .collect::<PathBuf>()
                    ),
                "Invalid or duplicate scaffold target: {}",
                entry.path.display()
            );
            if entry.original.is_none() {
                ensure!(
                    entry.path.starts_with(&journal.module_dir) && entry.path != journal.module_dir,
                    "Create target escapes its module"
                );
            } else {
                ensure!(
                    super::registration::is_registration_target(&entry.path),
                    "Recovery target is not an owner registration file: {}",
                    entry.path.display()
                );
            }
            ensure_no_symlink_ancestors(&self.root, &self.root.join(&entry.path))?;
        }
        for path in &seen {
            ensure!(
                !path.ancestors().skip(1).any(|parent| seen.contains(parent)),
                "Scaffold file is also a target directory"
            );
        }
        Ok(())
    }

    pub(super) fn apply(
        self,
        plan: &[PlannedFile],
        module_dir: &Path,
    ) -> Result<ScaffoldTransaction> {
        self.apply_with_writer(plan, module_dir, |file, bytes| file.write(bytes))
    }

    fn apply_with_writer(
        self,
        plan: &[PlannedFile],
        module_dir: &Path,
        mut write: impl FnMut(&mut File, &[u8]) -> std::io::Result<usize>,
    ) -> Result<ScaffoldTransaction> {
        ensure!(
            !present(&self.state.join("active"))?,
            "Recover the previous scaffold operation first"
        );
        let journal = Journal {
            version: JOURNAL_VERSION,
            workspace: self.root.canonicalize()?,
            module_dir: module_dir.strip_prefix(&self.root)?.to_path_buf(),
            files: plan
                .iter()
                .map(|entry| {
                    Ok(JournalFile {
                        path: entry.path.strip_prefix(&self.root)?.to_path_buf(),
                        original: entry.original.clone().map(String::from_utf8).transpose()?,
                        content: entry.content.clone(),
                    })
                })
                .collect::<Result<_>>()?,
        };
        self.validate_journal(&journal)?;
        ensure!(
            !present(module_dir)?,
            "Module directory already exists: {}",
            module_dir.display()
        );
        for entry in plan {
            ensure!(
                read_regular(&self.root, &entry.path)? == entry.original,
                "Scaffold target changed since planning: {}",
                entry.path.display()
            );
            if entry.original.is_some() {
                ensure!(
                    !fs::metadata(&entry.path)?.permissions().readonly(),
                    "Scaffold target is read-only: {}",
                    entry.path.display()
                );
            }
        }
        let preparing = self.state.join("preparing");
        let preparation = (|| -> Result<String> {
            fs::create_dir(&preparing)?;
            fs::create_dir(preparing.join("payload"))?;
            fs::create_dir(preparing.join("backup"))?;
            fs::create_dir(preparing.join("restore"))?;
            let bytes = serde_json::to_vec(&journal)?;
            ensure!(
                bytes.len() as u64 <= MAX_JOURNAL_BYTES,
                "Scaffold plan exceeds journal size limit"
            );
            write_synced(&preparing.join("plan.json"), &bytes)?;
            for (index, entry) in journal.files.iter().enumerate() {
                let payload = preparing.join(format!("payload/{index}"));
                let mut file = File::create_new(&payload)?;
                let mut remaining = entry.content.as_bytes();
                while !remaining.is_empty() {
                    match write(&mut file, remaining) {
                        Ok(0) => {
                            return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into());
                        }
                        Ok(count) => remaining = &remaining[count..],
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(error) => return Err(error.into()),
                    }
                    #[cfg(test)]
                    tests::checkpoint("staging_write");
                }
                if let Some(original) = &entry.original {
                    let backup = preparing.join(format!("backup/{index}"));
                    write_synced(&backup, original.as_bytes())?;
                    let permissions = fs::metadata(self.root.join(&entry.path))?.permissions();
                    fs::set_permissions(&backup, permissions.clone())?;
                    fs::set_permissions(&payload, permissions)?;
                    sync_file(&backup)?;
                }
                file.sync_all()?;
            }
            fs::rename(&preparing, self.state.join("active"))?;
            Ok(hex::encode(Sha256::digest(bytes)))
        })();
        let digest = match preparation {
            Ok(digest) => digest,
            Err(error) => {
                return match self.clean_private("preparing") {
                    Ok(()) => Err(error),
                    Err(cleanup) => Err(error)
                        .context(format!("Scaffold preparation cleanup failed: {cleanup}")),
                };
            }
        };
        #[cfg(test)]
        tests::checkpoint("prepared");
        let transaction = ScaffoldTransaction {
            workspace: self,
            journal,
            digest,
        };
        if let Err(error) = transaction.install() {
            return match transaction.rollback() {
                Ok(()) => Err(error),
                Err(recovery) => {
                    Err(error).context(format!("Scaffold recovery required: {recovery}"))
                }
            };
        }
        Ok(transaction)
    }

    fn restore(&self, journal: &Journal) -> Result<()> {
        let module_dir = self.root.join(&journal.module_dir);
        let claimed = read_regular(&self.root, &self.state.join("active/claimed"))?;
        ensure!(
            claimed.as_deref().is_none_or(|bytes| bytes == b"claimed"),
            "Invalid scaffold claim marker; preserve the journal for reconciliation"
        );
        if present(&module_dir)? {
            ensure!(
                claimed.as_deref() == Some(b"claimed"),
                "Ambiguous module directory claim; preserve {} and reconcile before scaffold --recover",
                module_dir.display()
            );
            self.inspect_module(journal, &module_dir)?;
        }
        for entry in &journal.files {
            let actual = read_regular(&self.root, &self.root.join(&entry.path))?;
            let original = entry.original.as_ref().map(|bytes| bytes.as_bytes());
            ensure!(
                actual.as_deref() == original
                    || actual.as_deref() == Some(entry.content.as_bytes()),
                "Rollback preserved changed target: {}; reconcile it before scaffold --recover",
                entry.path.display()
            );
        }
        for (index, entry) in journal.files.iter().enumerate().rev() {
            let path = self.root.join(&entry.path);
            let actual = read_regular(&self.root, &path)?;
            if actual.as_deref() == entry.original.as_ref().map(|value| value.as_bytes()) {
                continue;
            }
            if entry.original.is_some() {
                let restore = self.state.join(format!("active/restore/{index}"));
                self.remove_control_file(&restore)?;
                fs::copy(self.state.join(format!("active/backup/{index}")), &restore)?;
                sync_file(&restore)?;
                ensure!(
                    read_regular(&self.root, &restore)?.as_deref()
                        == entry.original.as_ref().map(|value| value.as_bytes()),
                    "Scaffold backup changed while restoring {}",
                    path.display()
                );
                ensure!(
                    read_regular(&self.root, &path)?.as_deref() == Some(entry.content.as_bytes()),
                    "Rollback preserved changed target: {}; reconcile it before scaffold --recover",
                    path.display()
                );
                fs::rename(restore, &path)?;
            } else {
                ensure!(
                    actual.as_deref() == Some(entry.content.as_bytes()),
                    "Rollback preserved changed target: {}; reconcile it before scaffold --recover",
                    path.display()
                );
                fs::remove_file(&path)?;
            }
            #[cfg(test)]
            tests::checkpoint("restored");
        }
        let mut directories = HashSet::new();
        for entry in &journal.files {
            if entry.original.is_none() {
                let mut parent = entry.path.parent();
                while let Some(directory) =
                    parent.filter(|path| path.starts_with(&journal.module_dir))
                {
                    directories.insert(directory.to_path_buf());
                    parent = directory.parent();
                }
            }
        }
        if claimed.is_some() {
            directories.insert(journal.module_dir.clone());
        }
        let mut directories: Vec<_> = directories.into_iter().collect();
        directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        for directory in directories {
            let path = self.root.join(directory);
            ensure_no_symlink_ancestors(&self.root, &path)?;
            if present(&path)? {
                fs::remove_dir(path)?;
            }
        }
        Ok(())
    }

    fn inspect_module(&self, journal: &Journal, directory: &Path) -> Result<()> {
        ensure_no_symlink_ancestors(&self.root, directory)?;
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            ensure_no_symlink_ancestors(&self.root, &path)?;
            let relative = path.strip_prefix(&self.root)?;
            if path.is_dir() {
                ensure!(
                    journal
                        .files
                        .iter()
                        .any(|entry| entry.original.is_none() && entry.path.starts_with(relative)),
                    "Rollback preserved foreign directory: {}",
                    path.display()
                );
                self.inspect_module(journal, &path)?;
            } else {
                ensure!(
                    journal
                        .files
                        .iter()
                        .any(|entry| entry.original.is_none() && entry.path == relative),
                    "Rollback preserved foreign file: {}",
                    path.display()
                );
            }
        }
        Ok(())
    }

    fn retire(&self) -> Result<()> {
        #[cfg(test)]
        tests::checkpoint("before_retire");
        fs::rename(self.state.join("active"), self.state.join("retired"))?;
        #[cfg(test)]
        tests::checkpoint("retired");
        self.clean_private("retired")
    }

    fn remove_control_file(&self, path: &Path) -> Result<()> {
        ensure_no_symlink_ancestors(&self.root, path)?;
        if present(path)? {
            ensure!(
                fs::symlink_metadata(path)?.is_file(),
                "Recovery control path is not a regular file: {}",
                path.display()
            );
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn clean_private(&self, name: &str) -> Result<()> {
        let directory = self.state.join(name);
        ensure_no_symlink_ancestors(&self.root, &directory)?;
        if !present(&directory)? {
            return Ok(());
        }
        // Only recognized private control files are disposable. Foreign entries block cleanup.
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            ensure_no_symlink_ancestors(&self.root, &path)?;
            let leaf = path
                .file_name()
                .and_then(|name| name.to_str())
                .context("Invalid recovery state path")?;
            if ["payload", "backup", "restore"].contains(&leaf) && path.is_dir() {
                for child in fs::read_dir(&path)? {
                    let child = child?.path();
                    ensure!(
                        child
                            .file_name()
                            .and_then(|name| name.to_str())
                            .and_then(|name| name.parse::<usize>().ok().map(|index| (name, index)))
                            .is_some_and(
                                |(name, index)| index < MAX_FILES && index.to_string() == name
                            ),
                        "Foreign recovery control file: {}",
                        child.display()
                    );
                    self.remove_control_file(&child)?;
                }
                fs::remove_dir(path)?;
            } else {
                ensure!(
                    ["plan.json", "claimed", "committed", "decision.pending"].contains(&leaf),
                    "Foreign recovery control entry: {}",
                    path.display()
                );
                self.remove_control_file(&path)?;
            }
        }
        fs::remove_dir(directory)?;
        Ok(())
    }
}

impl ScaffoldTransaction {
    fn install(&self) -> Result<()> {
        let root = &self.workspace.root;
        let module = root.join(&self.journal.module_dir);
        ensure_no_symlink_ancestors(root, &module)?;
        fs::create_dir(&module)?;
        #[cfg(test)]
        tests::checkpoint("unclaimed");
        write_synced(&self.workspace.state.join("active/claimed"), b"claimed")?;
        #[cfg(test)]
        tests::checkpoint("claimed");
        for (index, entry) in self.journal.files.iter().enumerate() {
            let path = root.join(&entry.path);
            ensure!(
                read_regular(root, &path)?.as_deref()
                    == entry.original.as_ref().map(|value| value.as_bytes()),
                "Scaffold target changed since planning: {}",
                path.display()
            );
            if entry.original.is_none() {
                let parent = path.parent().context("Missing source parent")?;
                fs::create_dir_all(parent)?;
                ensure_no_symlink_ancestors(root, &path)?;
                fs::hard_link(
                    self.workspace.state.join(format!("active/payload/{index}")),
                    &path,
                )
                .with_context(|| {
                    format!(
                        "Cannot publish complete scaffold file without clobbering: {}",
                        path.display()
                    )
                })?;
            } else {
                fs::rename(
                    self.workspace.state.join(format!("active/payload/{index}")),
                    &path,
                )?;
            }
            #[cfg(test)]
            tests::checkpoint(if entry.original.is_some() {
                "registration"
            } else {
                "source"
            });
        }
        Ok(())
    }

    pub(super) fn rollback(self) -> Result<()> {
        self.workspace.recover()
    }

    pub(super) fn commit(self) -> Result<()> {
        #[cfg(test)]
        tests::checkpoint("before_commit");
        let decision = (|| -> Result<()> {
            let plan = read_regular(
                &self.workspace.root,
                &self.workspace.state.join("active/plan.json"),
            )?
            .context("Missing scaffold recovery plan")?;
            ensure!(
                hex::encode(Sha256::digest(plan)) == self.digest,
                "Scaffold recovery plan changed before commit"
            );
            for entry in &self.journal.files {
                let path = self.workspace.root.join(&entry.path);
                ensure!(
                    read_regular(&self.workspace.root, &path)?.as_deref()
                        == Some(entry.content.as_bytes()),
                    "Scaffold target changed before commit: {}",
                    path.display()
                );
            }
            let temporary = self.workspace.state.join("active/decision.pending");
            write_synced(&temporary, self.digest.as_bytes())?;
            fs::rename(temporary, self.workspace.state.join("active/committed"))?;
            Ok(())
        })();
        if let Err(error) = decision {
            return match self.workspace.recover() {
                Ok(()) => Err(error).context("Scaffold commit failed; recovery completed"),
                Err(recovery) => Err(error).context(format!("Scaffold commit failed; run scaffold --recover after reconciliation: {recovery}")),
            };
        }
        #[cfg(test)]
        tests::checkpoint("committed");
        self.workspace.retire().context(
            "Scaffold committed; recovery-state cleanup is required before the next operation",
        )
    }

    #[cfg(test)]
    pub(super) fn apply(root: &Path, plan: &[PlannedFile], module_dir: &Path) -> Result<Self> {
        ScaffoldWorkspace::acquire(root)?.apply(plan, module_dir)
    }

    #[cfg(test)]
    pub(super) fn apply_with_writer(
        root: &Path,
        plan: &[PlannedFile],
        module_dir: &Path,
        write: impl FnMut(&mut File, &[u8]) -> std::io::Result<usize>,
    ) -> Result<Self> {
        ScaffoldWorkspace::acquire(root)?.apply_with_writer(plan, module_dir, write)
    }
}

#[cfg(test)]
mod tests;
