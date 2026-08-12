use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const INSTANCE_LAYOUT_REVISION: u8 = 1;

/// Host-local placement of one RusToK installation.
///
/// The path is deliberately not a release, artifact, object, or migration
/// identity. Hosts bind it before preflight; durable platform identity remains
/// in PostgreSQL and content-addressed receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstancePlacement {
    pub instance_id: Uuid,
    pub root: String,
}

impl InstancePlacement {
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            instance_id: Uuid::new_v4(),
            root: root.into(),
        }
    }

    pub fn validate(&self) -> Result<(), InstanceLayoutError> {
        if self.root.trim().is_empty() {
            return Err(InstanceLayoutError::InvalidRoot(
                "instance root is required".to_string(),
            ));
        }
        if self.root.contains('\0') {
            return Err(InstanceLayoutError::InvalidRoot(
                "instance root contains a NUL character".to_string(),
            ));
        }
        Ok(())
    }
}

/// Deterministic portable layout rooted at an operator-selected directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceLayout {
    placement: InstancePlacement,
    root: PathBuf,
}

impl InstanceLayout {
    pub fn resolve(
        placement: InstancePlacement,
        invocation_dir: impl AsRef<Path>,
    ) -> Result<Self, InstanceLayoutError> {
        placement.validate()?;
        let root = PathBuf::from(placement.root.trim());
        let absolute = if root.is_absolute() {
            root
        } else {
            invocation_dir.as_ref().join(root)
        };
        let root = canonicalize_existing_prefix(&normalize_absolute_path(&absolute)?)?;
        Ok(Self {
            placement: InstancePlacement {
                instance_id: placement.instance_id,
                root: root.to_string_lossy().into_owned(),
            },
            root,
        })
    }

    pub fn placement(&self) -> &InstancePlacement {
        &self.placement
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve_relative(
        &self,
        relative: impl AsRef<Path>,
    ) -> Result<PathBuf, InstanceLayoutError> {
        let relative = relative.as_ref();
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(InstanceLayoutError::InvalidRelativePath {
                path: relative.display().to_string(),
            });
        }
        Ok(self.root.join(relative))
    }

    pub fn marker(&self) -> PathBuf {
        self.state().join("instance.json")
    }

    pub fn config(&self) -> PathBuf {
        self.root.join("config")
    }

    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }

    pub fn operations(&self) -> PathBuf {
        self.root.join("operations")
    }

    pub fn operations_releases(&self) -> PathBuf {
        self.operations().join("releases/sha256")
    }

    pub fn operations_release(
        &self,
        digest: &str,
        target: &str,
    ) -> Result<PathBuf, InstanceLayoutError> {
        Ok(self
            .operations_releases()
            .join(sha256_hex(digest)?)
            .join(path_segment("operations target", target)?))
    }

    pub fn releases(&self) -> PathBuf {
        self.root.join("releases")
    }

    pub fn platform_releases(&self) -> PathBuf {
        self.releases().join("platform/sha256")
    }

    pub fn platform_bundle(&self, digest: &str) -> Result<PathBuf, InstanceLayoutError> {
        Ok(self.platform_releases().join(sha256_hex(digest)?))
    }

    pub fn platform_role(
        &self,
        bundle_digest: &str,
        role: &str,
    ) -> Result<PathBuf, InstanceLayoutError> {
        Ok(self
            .platform_bundle(bundle_digest)?
            .join(path_segment("platform role", role)?))
    }

    pub fn sources(&self) -> PathBuf {
        self.root.join("sources")
    }

    pub fn source_objects(&self) -> PathBuf {
        self.sources().join("objects")
    }

    pub fn source_object(&self, digest: &str) -> Result<PathBuf, InstanceLayoutError> {
        sharded_digest_path(&self.source_objects(), digest)
    }

    pub fn source_receipts(&self) -> PathBuf {
        self.sources().join("receipts")
    }

    pub fn source_receipt(&self, digest: &str) -> Result<PathBuf, InstanceLayoutError> {
        sharded_digest_path(&self.source_receipts(), digest)
    }

    pub fn storage(&self) -> PathBuf {
        self.root.join("storage")
    }

    pub fn data(&self) -> PathBuf {
        self.root.join("data")
    }

    pub fn service_data(&self) -> PathBuf {
        self.data().join("services")
    }

    pub fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn deployment_slots(&self) -> PathBuf {
        self.state().join("deployment/slots")
    }

    pub fn deployment_journal(&self) -> PathBuf {
        self.state().join("deployment/journal")
    }

    pub fn operations_slots(&self) -> PathBuf {
        self.state().join("operations/slots")
    }

    pub fn operations_journal(&self) -> PathBuf {
        self.state().join("operations/journal")
    }

    pub fn work(&self) -> PathBuf {
        self.root.join("work")
    }

    pub fn static_distribution_work(&self) -> PathBuf {
        self.work().join("static-distribution")
    }

    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn deployment_cache(&self) -> PathBuf {
        self.cache().join("deployment")
    }

    pub fn deployment_cache_entry(&self, digest: &str) -> Result<PathBuf, InstanceLayoutError> {
        sharded_digest_path(&self.deployment_cache(), digest)
    }

    pub fn module_runtime_cache(&self) -> PathBuf {
        self.cache().join("module-runtime")
    }

    pub fn module_payload_cache_entry(&self, digest: &str) -> Result<PathBuf, InstanceLayoutError> {
        sharded_digest_path(&self.module_runtime_cache().join("payload"), digest)
    }

    pub fn prepared_module_cache_entry(
        &self,
        runtime_fingerprint: &str,
        payload_digest: &str,
    ) -> Result<PathBuf, InstanceLayoutError> {
        Ok(self
            .module_runtime_cache()
            .join("prepared")
            .join(sha256_hex(runtime_fingerprint)?)
            .join(sha256_hex(payload_digest)?))
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn run(&self) -> PathBuf {
        self.root.join("run")
    }

    fn managed_directories(&self) -> Vec<PathBuf> {
        [
            self.bin(),
            self.config(),
            self.operations_releases(),
            self.platform_releases(),
            self.source_objects(),
            self.source_receipts(),
            self.storage(),
            self.service_data(),
            self.deployment_slots(),
            self.deployment_journal(),
            self.operations_slots(),
            self.operations_journal(),
            self.static_distribution_work(),
            self.deployment_cache(),
            self.module_runtime_cache(),
            self.logs(),
            self.run(),
        ]
        .into_iter()
        .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceLayoutMarker {
    pub layout_revision: u8,
    pub instance_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceLayoutPreparation {
    pub root: String,
    pub marker: String,
    pub created: bool,
    pub resumed: bool,
}

#[derive(Debug, Error)]
pub enum InstanceLayoutError {
    #[error("invalid instance root: {0}")]
    InvalidRoot(String),
    #[error("invalid instance-relative path `{path}`")]
    InvalidRelativePath { path: String },
    #[error("invalid canonical SHA-256 digest `{value}`")]
    InvalidDigest { value: String },
    #[error("invalid {name} path segment `{value}`")]
    InvalidPathSegment { name: &'static str, value: String },
    #[error("managed instance path `{path}` must not be a symbolic link or junction")]
    UnsafeManagedLink { path: String },
    #[error("instance root `{root}` is not empty and has no matching RusToK marker")]
    UnownedRoot { root: String },
    #[error("instance root `{root}` overlaps an existing RusToK instance at `{owner_root}`")]
    OverlappingRoot { root: String, owner_root: String },
    #[error("instance root marker does not match instance {expected}")]
    MarkerMismatch { expected: Uuid },
    #[error("unsupported instance layout revision `{0}`")]
    UnsupportedRevision(u8),
    #[error("instance layout I/O failed for `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid instance layout marker `{path}`: {source}")]
    InvalidMarker {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

fn sha256_hex(value: &str) -> Result<&str, InstanceLayoutError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(InstanceLayoutError::InvalidDigest {
            value: value.to_string(),
        });
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(InstanceLayoutError::InvalidDigest {
            value: value.to_string(),
        });
    }
    Ok(hex)
}

fn sharded_digest_path(root: &Path, digest: &str) -> Result<PathBuf, InstanceLayoutError> {
    let hex = sha256_hex(digest)?;
    Ok(root
        .join("sha256")
        .join(&hex[..2])
        .join(&hex[2..4])
        .join(hex))
}

fn path_segment<'a>(name: &'static str, value: &'a str) -> Result<&'a str, InstanceLayoutError> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(InstanceLayoutError::InvalidPathSegment {
            name,
            value: value.to_string(),
        });
    }
    Ok(value)
}

fn normalize_absolute_path(path: &Path) -> Result<PathBuf, InstanceLayoutError> {
    if !path.is_absolute() {
        return Err(InstanceLayoutError::InvalidRoot(format!(
            "resolved instance root `{}` is not absolute",
            path.display()
        )));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(InstanceLayoutError::InvalidRoot(format!(
                        "instance root `{}` escapes its filesystem root",
                        path.display()
                    )));
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

fn canonicalize_existing_prefix(path: &Path) -> Result<PathBuf, InstanceLayoutError> {
    let mut existing = path;
    let mut missing = Vec::new();
    while !existing.exists() {
        let name = existing.file_name().ok_or_else(|| {
            InstanceLayoutError::InvalidRoot(format!(
                "instance root `{}` has no existing filesystem prefix",
                path.display()
            ))
        })?;
        missing.push(name.to_os_string());
        existing = existing.parent().ok_or_else(|| {
            InstanceLayoutError::InvalidRoot(format!(
                "instance root `{}` has no existing filesystem prefix",
                path.display()
            ))
        })?;
    }
    let mut canonical = portable_canonical_path(
        std::fs::canonicalize(existing).map_err(|source| io_error(existing, source))?,
    );
    for component in missing.into_iter().rev() {
        canonical.push(component);
    }
    normalize_absolute_path(&canonical)
}

fn portable_canonical_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let rendered = path.to_string_lossy();
        if let Some(rest) = rendered.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = rendered.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

pub fn bind_instance_placement(
    root: impl Into<String>,
    invocation_dir: impl AsRef<Path>,
) -> Result<InstancePlacement, InstanceLayoutError> {
    let provisional = InstancePlacement::new(root);
    let layout = InstanceLayout::resolve(provisional, invocation_dir)?;
    for ancestor in layout.root().ancestors().skip(1) {
        let marker_path = ancestor.join("state/instance.json");
        if marker_path.is_file() {
            return Err(InstanceLayoutError::OverlappingRoot {
                root: layout.root().display().to_string(),
                owner_root: ancestor.display().to_string(),
            });
        }
    }
    if layout.marker().exists() {
        let marker = read_marker_file(&layout.marker())?;
        if marker.layout_revision != INSTANCE_LAYOUT_REVISION {
            return Err(InstanceLayoutError::UnsupportedRevision(
                marker.layout_revision,
            ));
        }
        return Ok(InstancePlacement {
            instance_id: marker.instance_id,
            root: layout.root().display().to_string(),
        });
    }
    if layout.root().is_dir() {
        let mut pending_markers = std::fs::read_dir(layout.root())
            .map_err(|source| io_error(layout.root(), source))?
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.starts_with(".rustok-instance.") && name.ends_with(".pending")
            });
        if let Some(entry) = pending_markers.next() {
            if pending_markers.next().is_some() {
                return Err(InstanceLayoutError::InvalidRoot(format!(
                    "instance root `{}` contains multiple pending instance markers",
                    layout.root().display()
                )));
            }
            let marker = read_marker_file(&entry.path())?;
            if marker.layout_revision != INSTANCE_LAYOUT_REVISION {
                return Err(InstanceLayoutError::UnsupportedRevision(
                    marker.layout_revision,
                ));
            }
            return Ok(InstancePlacement {
                instance_id: marker.instance_id,
                root: layout.root().display().to_string(),
            });
        }
    }
    if layout.root().exists() {
        if !layout.root().is_dir() {
            return Err(InstanceLayoutError::InvalidRoot(format!(
                "`{}` is not a directory",
                layout.root().display()
            )));
        }
        if directory_has_entries(layout.root())? {
            return Err(InstanceLayoutError::UnownedRoot {
                root: layout.root().display().to_string(),
            });
        }
    }
    Ok(layout.placement().clone())
}

/// Opens a layout for read/path resolution without claiming an unowned root.
///
/// A prepared instance reuses its durable marker identity. An unprepared root
/// receives an invocation-local identity that must never be persisted; only
/// [`bind_instance_placement`] and [`prepare_instance_layout`] may claim it.
pub fn inspect_instance_layout(
    root: impl Into<String>,
    invocation_dir: impl AsRef<Path>,
) -> Result<InstanceLayout, InstanceLayoutError> {
    let provisional = InstancePlacement::new(root);
    let layout = InstanceLayout::resolve(provisional, invocation_dir)?;
    for ancestor in layout.root().ancestors().skip(1) {
        let marker_path = ancestor.join("state/instance.json");
        if marker_path.is_file() {
            return Err(InstanceLayoutError::OverlappingRoot {
                root: layout.root().display().to_string(),
                owner_root: ancestor.display().to_string(),
            });
        }
    }
    if !layout.marker().exists() {
        return Ok(layout);
    }
    let marker = read_marker_file(&layout.marker())?;
    if marker.layout_revision != INSTANCE_LAYOUT_REVISION {
        return Err(InstanceLayoutError::UnsupportedRevision(
            marker.layout_revision,
        ));
    }
    InstanceLayout::resolve(
        InstancePlacement {
            instance_id: marker.instance_id,
            root: layout.root().display().to_string(),
        },
        layout.root(),
    )
}

pub fn prepare_instance_layout(
    layout: &InstanceLayout,
) -> Result<InstanceLayoutPreparation, InstanceLayoutError> {
    use std::fs::{self, OpenOptions};
    use std::io::Write;

    let root_existed = layout.root().exists();
    if root_existed && !layout.root().is_dir() {
        return Err(InstanceLayoutError::InvalidRoot(format!(
            "`{}` is not a directory",
            layout.root().display()
        )));
    }

    if let Some(marker) = read_marker(layout)? {
        validate_marker(layout, &marker)?;
        create_managed_directories(layout)?;
        return Ok(preparation(layout, false, true));
    }

    if root_existed && directory_has_entries(layout.root())? {
        return Err(InstanceLayoutError::UnownedRoot {
            root: layout.root().display().to_string(),
        });
    }

    fs::create_dir_all(layout.root()).map_err(|source| io_error(layout.root(), source))?;
    let pending = layout.root().join(format!(
        ".rustok-instance.{}.pending",
        layout.placement().instance_id
    ));
    let marker = InstanceLayoutMarker {
        layout_revision: INSTANCE_LAYOUT_REVISION,
        instance_id: layout.placement().instance_id,
    };
    let bytes = serde_json::to_vec_pretty(&marker).expect("instance marker must serialize");
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
    {
        Ok(mut file) => {
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|source| io_error(&pending, source))?;
        }
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = read_marker_file(&pending)?;
            validate_marker(layout, &existing)?;
        }
        Err(source) => return Err(io_error(&pending, source)),
    }
    fs::create_dir_all(layout.state()).map_err(|source| io_error(layout.state(), source))?;
    fs::rename(&pending, layout.marker()).map_err(|source| io_error(layout.marker(), source))?;
    create_managed_directories(layout)?;
    Ok(preparation(layout, !root_existed, false))
}

fn read_marker(
    layout: &InstanceLayout,
) -> Result<Option<InstanceLayoutMarker>, InstanceLayoutError> {
    if layout.marker().exists() {
        return read_marker_file(&layout.marker()).map(Some);
    }
    let pending = layout.root().join(format!(
        ".rustok-instance.{}.pending",
        layout.placement().instance_id
    ));
    if pending.exists() {
        let marker = read_marker_file(&pending)?;
        validate_marker(layout, &marker)?;
        std::fs::create_dir_all(layout.state())
            .map_err(|source| io_error(layout.state(), source))?;
        std::fs::rename(&pending, layout.marker())
            .map_err(|source| io_error(layout.marker(), source))?;
        return Ok(Some(marker));
    }
    Ok(None)
}

fn read_marker_file(path: &Path) -> Result<InstanceLayoutMarker, InstanceLayoutError> {
    let bytes = std::fs::read(path).map_err(|source| io_error(path, source))?;
    serde_json::from_slice(&bytes).map_err(|source| InstanceLayoutError::InvalidMarker {
        path: path.display().to_string(),
        source,
    })
}

fn validate_marker(
    layout: &InstanceLayout,
    marker: &InstanceLayoutMarker,
) -> Result<(), InstanceLayoutError> {
    if marker.layout_revision != INSTANCE_LAYOUT_REVISION {
        return Err(InstanceLayoutError::UnsupportedRevision(
            marker.layout_revision,
        ));
    }
    if marker.instance_id != layout.placement().instance_id {
        return Err(InstanceLayoutError::MarkerMismatch {
            expected: layout.placement().instance_id,
        });
    }
    Ok(())
}

fn directory_has_entries(path: &Path) -> Result<bool, InstanceLayoutError> {
    std::fs::read_dir(path)
        .map_err(|source| io_error(path, source))?
        .next()
        .transpose()
        .map(|entry| entry.is_some())
        .map_err(|source| io_error(path, source))
}

fn create_managed_directories(layout: &InstanceLayout) -> Result<(), InstanceLayoutError> {
    for path in layout.managed_directories() {
        reject_managed_links(layout.root(), &path)?;
        std::fs::create_dir_all(&path).map_err(|source| io_error(path, source))?;
    }
    Ok(())
}

fn reject_managed_links(root: &Path, path: &Path) -> Result<(), InstanceLayoutError> {
    let relative =
        path.strip_prefix(root)
            .map_err(|_| InstanceLayoutError::InvalidRelativePath {
                path: path.display().to_string(),
            })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if is_managed_link(&metadata) => {
                return Err(InstanceLayoutError::UnsafeManagedLink {
                    path: current.display().to_string(),
                });
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(source) => return Err(io_error(&current, source)),
        }
    }
    Ok(())
}

fn is_managed_link(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        // Directory junctions are reparse points but are not guaranteed to be
        // reported as symbolic links by every Windows filesystem/provider.
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

fn preparation(layout: &InstanceLayout, created: bool, resumed: bool) -> InstanceLayoutPreparation {
    InstanceLayoutPreparation {
        root: layout.root().display().to_string(),
        marker: layout.marker().display().to_string(),
        created,
        resumed,
    }
}

fn io_error(path: impl AsRef<Path>, source: std::io::Error) -> InstanceLayoutError {
    InstanceLayoutError::Io {
        path: path.as_ref().display().to_string(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InstanceLayout, InstanceLayoutError, InstancePlacement, bind_instance_placement,
        prepare_instance_layout,
    };

    #[test]
    fn relative_root_resolves_against_the_invocation_directory() {
        let base = std::env::temp_dir().join("rustok-layout-base");
        let layout = InstanceLayout::resolve(InstancePlacement::new("shop-a"), &base).unwrap();
        assert_eq!(layout.root(), base.join("shop-a"));
        assert_eq!(layout.storage(), base.join("shop-a/storage"));
        assert_eq!(layout.sources(), base.join("shop-a/sources"));
    }

    #[test]
    fn content_addressed_paths_are_portable_and_cannot_escape_the_root() {
        let base = std::env::temp_dir().join("rustok-layout-base");
        let layout = InstanceLayout::resolve(InstancePlacement::new("shop-a"), &base).unwrap();
        let digest = format!("sha256:{}", "ab".repeat(32));

        assert_eq!(
            layout.platform_role(&digest, "admin_ssr").unwrap(),
            base.join(format!(
                "shop-a/releases/platform/sha256/{}/admin_ssr",
                "ab".repeat(32)
            ))
        );
        assert_eq!(
            layout.source_object(&digest).unwrap(),
            base.join(format!(
                "shop-a/sources/objects/sha256/ab/ab/{}",
                "ab".repeat(32)
            ))
        );
        assert!(layout.platform_role(&digest, "../foreign").is_err());
        assert!(layout.source_object(&"AB".repeat(32)).is_err());
    }

    #[test]
    fn inspection_reuses_a_marker_but_does_not_claim_an_unprepared_root() {
        let parent = std::env::temp_dir().join(format!("rustok-layout-{}", uuid::Uuid::new_v4()));
        let unprepared_root = parent.join("unprepared");
        std::fs::create_dir_all(&unprepared_root).unwrap();
        std::fs::write(unprepared_root.join("developer.txt"), "keep").unwrap();

        let first =
            super::inspect_instance_layout(unprepared_root.display().to_string(), &parent).unwrap();
        assert!(!first.marker().exists());
        assert_eq!(
            std::fs::read_to_string(unprepared_root.join("developer.txt")).unwrap(),
            "keep"
        );

        let prepared_root = parent.join("prepared");
        let prepared = InstanceLayout::resolve(
            InstancePlacement::new(prepared_root.display().to_string()),
            &parent,
        )
        .unwrap();
        prepare_instance_layout(&prepared).unwrap();
        let inspected =
            super::inspect_instance_layout(prepared_root.display().to_string(), &parent).unwrap();
        assert_eq!(
            inspected.placement().instance_id,
            prepared.placement().instance_id
        );

        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn existing_parent_is_canonicalized_before_a_new_root_is_bound() {
        let parent = std::env::temp_dir().join(format!("rustok-layout-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&parent).unwrap();
        let input = parent.join("missing").join("..").join("shop-a");
        let layout =
            InstanceLayout::resolve(InstancePlacement::new(input.display().to_string()), &parent)
                .unwrap();
        let canonical_parent =
            super::portable_canonical_path(std::fs::canonicalize(&parent).unwrap());

        assert_eq!(layout.root(), canonical_parent.join("shop-a"));
        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn preparation_is_restart_safe_and_rejects_a_foreign_root() {
        let parent = std::env::temp_dir().join(format!("rustok-layout-{}", uuid::Uuid::new_v4()));
        let owned_root = parent.join("owned");
        let placement = InstancePlacement::new(owned_root.display().to_string());
        let layout = InstanceLayout::resolve(placement.clone(), &parent).unwrap();

        let first = prepare_instance_layout(&layout).unwrap();
        assert!(first.created);
        assert!(!first.resumed);
        assert!(layout.marker().is_file());
        assert!(layout.static_distribution_work().is_dir());

        let resumed = prepare_instance_layout(&layout).unwrap();
        assert!(!resumed.created);
        assert!(resumed.resumed);

        let other = InstanceLayout::resolve(
            InstancePlacement::new(owned_root.display().to_string()),
            &parent,
        )
        .unwrap();
        assert!(matches!(
            prepare_instance_layout(&other),
            Err(InstanceLayoutError::MarkerMismatch { .. })
        ));

        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn preparation_never_claims_a_nonempty_unmarked_directory() {
        let parent = std::env::temp_dir().join(format!("rustok-layout-{}", uuid::Uuid::new_v4()));
        let root = parent.join("foreign");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("keep.txt"), "user data").unwrap();
        let layout =
            InstanceLayout::resolve(InstancePlacement::new(root.display().to_string()), &parent)
                .unwrap();

        assert!(matches!(
            prepare_instance_layout(&layout),
            Err(InstanceLayoutError::UnownedRoot { .. })
        ));
        assert_eq!(
            std::fs::read_to_string(root.join("keep.txt")).unwrap(),
            "user data"
        );

        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn binding_rejects_a_root_nested_inside_an_existing_instance() {
        let parent = std::env::temp_dir().join(format!("rustok-layout-{}", uuid::Uuid::new_v4()));
        let owner = InstanceLayout::resolve(
            InstancePlacement::new(parent.join("owner").display().to_string()),
            &parent,
        )
        .unwrap();
        prepare_instance_layout(&owner).unwrap();

        assert!(matches!(
            bind_instance_placement(owner.root().join("nested").display().to_string(), &parent),
            Err(InstanceLayoutError::OverlappingRoot { .. })
        ));

        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn binding_resumes_an_exact_pending_marker_after_process_loss() {
        let parent = std::env::temp_dir().join(format!("rustok-layout-{}", uuid::Uuid::new_v4()));
        let root = parent.join("pending");
        std::fs::create_dir_all(&root).unwrap();
        let instance_id = uuid::Uuid::new_v4();
        let pending = root.join(format!(".rustok-instance.{instance_id}.pending"));
        std::fs::write(
            &pending,
            serde_json::to_vec(&super::InstanceLayoutMarker {
                layout_revision: super::INSTANCE_LAYOUT_REVISION,
                instance_id,
            })
            .unwrap(),
        )
        .unwrap();

        let placement =
            bind_instance_placement(root.display().to_string(), &parent).expect("resume binding");
        assert_eq!(placement.instance_id, instance_id);
        let layout = InstanceLayout::resolve(placement, &parent).unwrap();
        let resumed = prepare_instance_layout(&layout).unwrap();
        assert!(resumed.resumed);
        assert!(layout.marker().is_file());
        assert!(!pending.exists());

        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn independent_roots_support_multiple_instances_on_one_host() {
        let parent = std::env::temp_dir().join(format!("rustok-layout-{}", uuid::Uuid::new_v4()));
        let first = InstanceLayout::resolve(
            InstancePlacement::new(parent.join("shop-a").display().to_string()),
            &parent,
        )
        .unwrap();
        let second = InstanceLayout::resolve(
            InstancePlacement::new(parent.join("shop-b").display().to_string()),
            &parent,
        )
        .unwrap();

        prepare_instance_layout(&first).unwrap();
        prepare_instance_layout(&second).unwrap();

        assert_ne!(
            first.placement().instance_id,
            second.placement().instance_id
        );
        assert_ne!(first.root(), second.root());
        assert!(first.platform_releases().is_dir());
        assert!(second.platform_releases().is_dir());
        assert!(first.module_runtime_cache().is_dir());
        assert!(second.module_runtime_cache().is_dir());

        std::fs::remove_dir_all(parent).unwrap();
    }
}
