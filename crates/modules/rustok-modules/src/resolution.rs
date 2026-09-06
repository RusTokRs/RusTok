//! Deterministic PubGrub adapter for exact admitted module dependency resolution.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, VecDeque},
    convert::Infallible,
};

use async_trait::async_trait;
use pubgrub::{
    Dependencies, DependencyConstraints, DependencyProvider, PackageResolutionStatistics, Ranges,
    resolve,
};
use semver::{Comparator, Op, Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::{
    ArtifactModuleKind, ModuleDependencyConstraint, ModuleDependencyLockGraph,
    ModuleDependencyLockNode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleResolutionScope {
    Platform,
    Tenant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleResolutionProviderKind {
    Artifact,
    StaticCore,
}

/// Candidate released by the owner catalog and eligible for solver evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleResolutionCandidate {
    pub node: ModuleDependencyLockNode,
    pub runtime_abi: String,
    /// Semantic-version range from the admitted artifact descriptor.
    pub platform_compatibility: String,
    pub trusted: bool,
    pub active: bool,
    pub yanked: bool,
    pub revoked: bool,
    pub scope: ModuleResolutionScope,
    pub module_kind: ArtifactModuleKind,
    pub provider_kind: ModuleResolutionProviderKind,
    /// Constraints from the admitted descriptor for this exact release. The
    /// lock node intentionally stores only the selected dependency slugs.
    #[serde(default)]
    pub dependencies: Vec<ModuleDependencyConstraint>,
}

/// Owner provider. Its implementation may use `pubgrub`, but its dependency
/// solving types and derivation internals never cross this boundary.
#[async_trait]
pub trait ModuleResolutionProvider: Send + Sync {
    async fn candidates(
        &self,
        dependency: &ModuleDependencyConstraint,
    ) -> Result<Vec<ModuleResolutionCandidate>, ModuleResolutionError>;
}

/// Immutable inputs for one dependency solve. The catalog provider is queried
/// before PubGrub runs, so the solving algorithm never performs I/O and every
/// result can be reproduced from the admission snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleResolutionRequest {
    pub graph_revision: u64,
    pub runtime_abi: String,
    /// Exact platform release selected by deployment composition.
    pub platform_version: String,
    pub scope: ModuleResolutionScope,
    #[serde(default)]
    pub root_dependencies: Vec<ModuleDependencyConstraint>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleResolutionConflict {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub involved_slugs: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModuleResolutionError {
    #[error("module resolution provider failed: {0}")]
    Provider(String),
    #[error("module dependency resolution conflict: {conflict:?}")]
    Conflict { conflict: ModuleResolutionConflict },
    #[error("module resolution received an invalid candidate: {0}")]
    InvalidCandidate(String),
    #[error("module resolution received an invalid platform version `{0}`")]
    InvalidPlatformVersion(String),
    #[error("module resolution received an invalid platform compatibility range `{0}`")]
    InvalidPlatformCompatibility(String),
    #[error("module resolution does not support prerelease versions in v1: {0}")]
    UnsupportedPrerelease(String),
    #[error("module resolution cannot represent version requirement `{0}")]
    UnsupportedRequirement(String),
}

impl ModuleResolutionError {
    /// Stable transport-neutral conflict data. Solver diagnostics intentionally
    /// remain private implementation detail and must not become an API.
    pub fn conflict(&self) -> Option<&ModuleResolutionConflict> {
        match self {
            Self::Conflict { conflict } => Some(conflict),
            _ => None,
        }
    }
}

/// Immutable result returned by the solver adapter after selecting every
/// direct and transitive dependency for one scope/revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleResolutionResult {
    pub lock_graph: ModuleDependencyLockGraph,
    #[serde(default)]
    pub conflicts: Vec<ModuleResolutionConflict>,
}

const ROOT_PACKAGE: &str = "__rustok_module_root__";
const ROOT_VERSION: Version = Version::new(0, 0, 0);

/// Resolves direct and transitive artifact dependencies with PubGrub, then
/// persists only the selected exact candidates in a tamper-evident lock graph.
pub async fn resolve_module_dependencies<P>(
    provider: &P,
    request: ModuleResolutionRequest,
) -> Result<ModuleResolutionResult, ModuleResolutionError>
where
    P: ModuleResolutionProvider,
{
    let platform_version = Version::parse(&request.platform_version).map_err(|_| {
        ModuleResolutionError::InvalidPlatformVersion(request.platform_version.clone())
    })?;
    let snapshot = ResolutionSnapshot::collect(provider, &request, &platform_version).await?;
    let selected = resolve(&snapshot, ROOT_PACKAGE.to_string(), ROOT_VERSION).map_err(|_| {
        ModuleResolutionError::Conflict {
            conflict: conflict_for_request(&request),
        }
    })?;
    let mut nodes = Vec::new();
    for (slug, version) in selected {
        if slug == ROOT_PACKAGE {
            continue;
        }
        let candidate = snapshot
            .candidate(&slug, &version)
            .expect("PubGrub may select only a candidate present in its immutable snapshot");
        nodes.push(candidate.node.clone());
    }
    let lock_graph = ModuleDependencyLockGraph::create(request.graph_revision, nodes)
        .map_err(|error| ModuleResolutionError::InvalidCandidate(error.to_string()))?;
    Ok(ModuleResolutionResult {
        lock_graph,
        conflicts: Vec::new(),
    })
}

fn conflict_for_request(request: &ModuleResolutionRequest) -> ModuleResolutionConflict {
    let involved_slugs = request
        .root_dependencies
        .iter()
        .map(|dependency| dependency.slug.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    ModuleResolutionConflict {
        code: "DEPENDENCY_CONFLICT".to_string(),
        message: "No compatible set of admitted module releases satisfies the requested dependency constraints."
            .to_string(),
        involved_slugs,
    }
}

#[derive(Debug)]
struct ResolutionSnapshot {
    releases: BTreeMap<String, BTreeMap<Version, ModuleResolutionCandidate>>,
    root_dependencies: DependencyConstraints<String, Ranges<Version>>,
}

impl ResolutionSnapshot {
    async fn collect<P>(
        provider: &P,
        request: &ModuleResolutionRequest,
        platform_version: &Version,
    ) -> Result<Self, ModuleResolutionError>
    where
        P: ModuleResolutionProvider,
    {
        let mut releases = BTreeMap::<String, BTreeMap<Version, ModuleResolutionCandidate>>::new();
        let mut queued = VecDeque::from(request.root_dependencies.clone());
        let mut requested = BTreeSet::new();
        while let Some(constraint) = queued.pop_front() {
            validate_constraint(&constraint)?;
            let request_key = format!("{}@{}", constraint.slug, constraint.version_requirement);
            if !requested.insert(request_key) {
                continue;
            }
            for candidate in provider.candidates(&constraint).await? {
                let version = candidate_version(&candidate)?;
                if !candidate_is_eligible(
                    &candidate,
                    &constraint,
                    request.scope,
                    &request.runtime_abi,
                    platform_version,
                    &version,
                )? {
                    continue;
                }
                validate_candidate(&candidate)?;
                let candidates = releases.entry(candidate.node.slug.clone()).or_default();
                if let Some(existing) = candidates.get(&version) {
                    if existing != &candidate {
                        return Err(ModuleResolutionError::InvalidCandidate(format!(
                            "multiple admitted releases exist for {} {version}",
                            candidate.node.slug
                        )));
                    }
                } else {
                    queued.extend(candidate.dependencies.clone());
                    candidates.insert(version, candidate);
                }
            }
        }
        Ok(Self {
            releases,
            root_dependencies: constraints_for(&request.root_dependencies)?,
        })
    }

    fn candidate(&self, slug: &str, version: &Version) -> Option<&ModuleResolutionCandidate> {
        self.releases.get(slug)?.get(version)
    }
}

impl DependencyProvider for ResolutionSnapshot {
    type P = String;
    type V = Version;
    type VS = Ranges<Version>;
    type Priority = Reverse<usize>;
    type M = String;
    type Err = Infallible;

    fn prioritize(
        &self,
        package: &Self::P,
        range: &Self::VS,
        _conflicts: &PackageResolutionStatistics,
    ) -> Self::Priority {
        Reverse(
            self.releases
                .get(package)
                .map(|versions| {
                    versions
                        .keys()
                        .filter(|version| range.contains(version))
                        .count()
                })
                .unwrap_or(0),
        )
    }

    fn choose_version(
        &self,
        package: &Self::P,
        range: &Self::VS,
    ) -> Result<Option<Self::V>, Self::Err> {
        Ok(if package == ROOT_PACKAGE {
            Some(ROOT_VERSION)
        } else {
            self.releases
                .get(package)
                .and_then(|versions| {
                    versions
                        .keys()
                        .rev()
                        .find(|version| range.contains(version))
                })
                .cloned()
        })
    }

    fn get_dependencies(
        &self,
        package: &Self::P,
        version: &Self::V,
    ) -> Result<Dependencies<Self::P, Self::VS, Self::M>, Self::Err> {
        if package == ROOT_PACKAGE && version == &ROOT_VERSION {
            return Ok(Dependencies::Available(self.root_dependencies.clone()));
        }
        let dependencies = self
            .candidate(package, version)
            .map(|candidate| constraints_for(&candidate.dependencies))
            .transpose()
            .expect("candidate dependencies are validated while creating the snapshot");
        Ok(match dependencies {
            Some(dependencies) => Dependencies::Available(dependencies),
            None => {
                Dependencies::Unavailable(format!("admitted release {package} {version} is absent"))
            }
        })
    }
}

fn candidate_is_eligible(
    candidate: &ModuleResolutionCandidate,
    constraint: &ModuleDependencyConstraint,
    scope: ModuleResolutionScope,
    runtime_abi: &str,
    platform_version: &Version,
    version: &Version,
) -> Result<bool, ModuleResolutionError> {
    let requirement = VersionReq::parse(&constraint.version_requirement).map_err(|_| {
        ModuleResolutionError::UnsupportedRequirement(constraint.version_requirement.clone())
    })?;
    let platform_compatibility =
        VersionReq::parse(&candidate.platform_compatibility).map_err(|_| {
            ModuleResolutionError::InvalidPlatformCompatibility(
                candidate.platform_compatibility.clone(),
            )
        })?;
    Ok(candidate.trusted
        && candidate.active
        && !candidate.yanked
        && !candidate.revoked
        && candidate.scope == scope
        && candidate.module_kind == ArtifactModuleKind::Optional
        && candidate.provider_kind == ModuleResolutionProviderKind::Artifact
        && candidate.runtime_abi == runtime_abi
        && platform_compatibility.matches(platform_version)
        && requirement.matches(version))
}

fn candidate_version(
    candidate: &ModuleResolutionCandidate,
) -> Result<Version, ModuleResolutionError> {
    let version = Version::parse(&candidate.node.version).map_err(|error| {
        ModuleResolutionError::InvalidCandidate(format!(
            "{} has invalid version: {error}",
            candidate.node.slug
        ))
    })?;
    if !version.pre.is_empty() {
        return Err(ModuleResolutionError::UnsupportedPrerelease(
            version.to_string(),
        ));
    }
    Ok(version)
}

fn validate_candidate(candidate: &ModuleResolutionCandidate) -> Result<(), ModuleResolutionError> {
    if candidate.node.slug == ROOT_PACKAGE || candidate.node.slug.trim().is_empty() {
        return Err(ModuleResolutionError::InvalidCandidate(
            "reserved or empty module slug".into(),
        ));
    }
    let declared = candidate
        .dependencies
        .iter()
        .map(|dependency| dependency.slug.as_str())
        .collect::<BTreeSet<_>>();
    let locked = candidate
        .node
        .dependencies
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if declared.len() != candidate.dependencies.len()
        || locked.len() != candidate.node.dependencies.len()
        || declared != locked
    {
        return Err(ModuleResolutionError::InvalidCandidate(format!(
            "{} {} has inconsistent dependency metadata",
            candidate.node.slug, candidate.node.version
        )));
    }
    for dependency in &candidate.dependencies {
        validate_constraint(dependency)?;
    }
    Ok(())
}

fn validate_constraint(
    constraint: &ModuleDependencyConstraint,
) -> Result<(), ModuleResolutionError> {
    if constraint.slug.trim().is_empty() || constraint.slug == ROOT_PACKAGE {
        return Err(ModuleResolutionError::InvalidCandidate(
            "reserved or empty dependency slug".into(),
        ));
    }
    let requirement = VersionReq::parse(&constraint.version_requirement).map_err(|_| {
        ModuleResolutionError::UnsupportedRequirement(constraint.version_requirement.clone())
    })?;
    if requirement
        .comparators
        .iter()
        .any(|comparator| !comparator.pre.is_empty())
    {
        return Err(ModuleResolutionError::UnsupportedPrerelease(
            constraint.version_requirement.clone(),
        ));
    }
    Ok(())
}

fn constraints_for(
    dependencies: &[ModuleDependencyConstraint],
) -> Result<DependencyConstraints<String, Ranges<Version>>, ModuleResolutionError> {
    // PubGrub 0.4 keeps constraints in an ordered vector so duplicate package
    // keys remain meaningful to the solver. Intersect duplicate declarations
    // here because the module descriptor contract treats them as cumulative.
    let mut merged = BTreeMap::<String, Ranges<Version>>::new();
    for dependency in dependencies {
        let range = version_requirement_range(&dependency.version_requirement)?;
        merged
            .entry(dependency.slug.clone())
            .and_modify(|existing| *existing = existing.intersection(&range))
            .or_insert(range);
    }
    Ok(merged.into_iter().collect())
}

fn version_requirement_range(requirement: &str) -> Result<Ranges<Version>, ModuleResolutionError> {
    let requirement = VersionReq::parse(requirement)
        .map_err(|_| ModuleResolutionError::UnsupportedRequirement(requirement.to_string()))?;
    requirement
        .comparators
        .iter()
        .try_fold(Ranges::full(), |range, comparator| {
            Ok(range.intersection(&comparator_range(comparator, requirement.to_string())?))
        })
}

fn comparator_range(
    comparator: &Comparator,
    original: String,
) -> Result<Ranges<Version>, ModuleResolutionError> {
    if !comparator.pre.is_empty() {
        return Err(ModuleResolutionError::UnsupportedPrerelease(original));
    }
    let base = Version::new(
        comparator.major,
        comparator.minor.unwrap_or(0),
        comparator.patch.unwrap_or(0),
    );
    let upper_exact = || upper_for_exact(comparator, &original);
    match comparator.op {
        Op::Exact | Op::Wildcard if comparator.minor.is_none() || comparator.patch.is_none() => {
            Ok(Ranges::between(base, upper_exact()?))
        }
        Op::Exact | Op::Wildcard => Ok(Ranges::singleton(base)),
        Op::Greater if comparator.minor.is_none() || comparator.patch.is_none() => {
            Ok(Ranges::higher_than(upper_exact()?))
        }
        Op::Greater => Ok(Ranges::strictly_higher_than(base)),
        Op::GreaterEq => Ok(Ranges::higher_than(base)),
        Op::Less => Ok(Ranges::strictly_lower_than(base)),
        Op::LessEq if comparator.minor.is_none() || comparator.patch.is_none() => {
            Ok(Ranges::strictly_lower_than(upper_exact()?))
        }
        Op::LessEq => Ok(Ranges::lower_than(base)),
        Op::Tilde => Ok(Ranges::between(
            base,
            upper_for_tilde(comparator, &original)?,
        )),
        Op::Caret => Ok(Ranges::between(
            base,
            upper_for_caret(comparator, &original)?,
        )),
        _ => Err(ModuleResolutionError::UnsupportedRequirement(original)),
    }
}

fn upper_for_exact(
    comparator: &Comparator,
    original: &str,
) -> Result<Version, ModuleResolutionError> {
    match (comparator.minor, comparator.patch) {
        (None, _) => next_major(comparator.major, original),
        (Some(minor), None) => next_minor(comparator.major, minor, original),
        (Some(_), Some(_)) => Err(ModuleResolutionError::UnsupportedRequirement(
            original.into(),
        )),
    }
}

fn upper_for_tilde(
    comparator: &Comparator,
    original: &str,
) -> Result<Version, ModuleResolutionError> {
    match comparator.minor {
        Some(minor) => next_minor(comparator.major, minor, original),
        None => next_major(comparator.major, original),
    }
}

fn upper_for_caret(
    comparator: &Comparator,
    original: &str,
) -> Result<Version, ModuleResolutionError> {
    match (comparator.major, comparator.minor, comparator.patch) {
        (major, _, _) if major > 0 => next_major(major, original),
        (0, Some(minor), _) if minor > 0 => next_minor(0, minor, original),
        (0, Some(0), Some(patch)) => Ok(Version::new(0, 0, next(patch, original)?)),
        (0, Some(0), None) | (0, None, _) => Ok(Version::new(0, 1, 0)),
        _ => Err(ModuleResolutionError::UnsupportedRequirement(
            original.into(),
        )),
    }
}

fn next_major(major: u64, original: &str) -> Result<Version, ModuleResolutionError> {
    Ok(Version::new(next(major, original)?, 0, 0))
}

fn next_minor(major: u64, minor: u64, original: &str) -> Result<Version, ModuleResolutionError> {
    Ok(Version::new(major, next(minor, original)?, 0))
}

fn next(value: u64, original: &str) -> Result<u64, ModuleResolutionError> {
    value
        .checked_add(1)
        .ok_or_else(|| ModuleResolutionError::UnsupportedRequirement(original.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModuleDependencyLockNode;

    #[derive(Default)]
    struct Catalog(BTreeMap<String, Vec<ModuleResolutionCandidate>>);

    #[async_trait]
    impl ModuleResolutionProvider for Catalog {
        async fn candidates(
            &self,
            dependency: &ModuleDependencyConstraint,
        ) -> Result<Vec<ModuleResolutionCandidate>, ModuleResolutionError> {
            Ok(self.0.get(&dependency.slug).cloned().unwrap_or_default())
        }
    }

    fn candidate(
        slug: &str,
        version: &str,
        dependencies: &[(&str, &str)],
    ) -> ModuleResolutionCandidate {
        ModuleResolutionCandidate {
            node: ModuleDependencyLockNode {
                slug: slug.into(),
                version: version.into(),
                payload_digest: format!("sha256:{}", "a".repeat(64)),
                manifest_digest: format!("sha256:{}", "b".repeat(64)),
                dependencies: dependencies
                    .iter()
                    .map(|(slug, _)| (*slug).into())
                    .collect(),
            },
            runtime_abi: "v1".into(),
            platform_compatibility: "^1.0".into(),
            trusted: true,
            active: true,
            yanked: false,
            revoked: false,
            scope: ModuleResolutionScope::Platform,
            module_kind: ArtifactModuleKind::Optional,
            provider_kind: ModuleResolutionProviderKind::Artifact,
            dependencies: dependencies
                .iter()
                .map(|(slug, version_requirement)| ModuleDependencyConstraint {
                    slug: (*slug).into(),
                    version_requirement: (*version_requirement).into(),
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn pubgrub_selects_a_compatible_transitive_release() {
        let mut catalog = Catalog::default();
        catalog.0.insert(
            "app".into(),
            vec![candidate("app", "1.0.0", &[("base", "^2")])],
        );
        catalog.0.insert(
            "base".into(),
            vec![
                candidate("base", "1.5.0", &[]),
                candidate("base", "2.1.0", &[]),
            ],
        );
        let result = resolve_module_dependencies(
            &catalog,
            ModuleResolutionRequest {
                graph_revision: 9,
                runtime_abi: "v1".into(),
                platform_version: "1.2.3".into(),
                scope: ModuleResolutionScope::Platform,
                root_dependencies: vec![ModuleDependencyConstraint {
                    slug: "app".into(),
                    version_requirement: "^1".into(),
                }],
            },
        )
        .await
        .expect("resolution succeeds");
        assert_eq!(result.lock_graph.nodes.len(), 2);
        assert!(
            result
                .lock_graph
                .nodes
                .iter()
                .any(|node| node.version == "2.1.0")
        );
    }

    #[tokio::test]
    async fn resolution_conflict_has_a_stable_transport_contract() {
        let error = resolve_module_dependencies(
            &Catalog::default(),
            ModuleResolutionRequest {
                graph_revision: 1,
                runtime_abi: "v1".into(),
                platform_version: "1.2.3".into(),
                scope: ModuleResolutionScope::Platform,
                root_dependencies: vec![ModuleDependencyConstraint {
                    slug: "missing".into(),
                    version_requirement: "^1".into(),
                }],
            },
        )
        .await
        .expect_err("missing release must conflict");
        let conflict = error.conflict().expect("stable conflict data");
        assert_eq!(conflict.code, "DEPENDENCY_CONFLICT");
        assert_eq!(conflict.involved_slugs, vec!["missing"]);
        assert!(!conflict.message.contains("pubgrub"));
    }

    #[tokio::test]
    async fn provider_excludes_platform_incompatible_candidates_before_pubgrub() {
        let mut catalog = Catalog::default();
        let mut incompatible = candidate("platform_dep", "2.0.0", &[]);
        incompatible.platform_compatibility = "^2.0".into();
        catalog.0.insert(
            "platform_dep".into(),
            vec![incompatible, candidate("platform_dep", "1.0.0", &[])],
        );

        let result = resolve_module_dependencies(
            &catalog,
            ModuleResolutionRequest {
                graph_revision: 2,
                runtime_abi: "v1".into(),
                platform_version: "1.2.3".into(),
                scope: ModuleResolutionScope::Platform,
                root_dependencies: vec![ModuleDependencyConstraint {
                    slug: "platform_dep".into(),
                    version_requirement: ">=1".into(),
                }],
            },
        )
        .await
        .expect("compatible platform release is selected");

        assert_eq!(result.lock_graph.nodes.len(), 1);
        assert_eq!(result.lock_graph.nodes[0].version, "1.0.0");
    }

    #[tokio::test]
    async fn malformed_platform_facts_fail_closed() {
        let error = resolve_module_dependencies(
            &Catalog::default(),
            ModuleResolutionRequest {
                graph_revision: 3,
                runtime_abi: "v1".into(),
                platform_version: "not-semver".into(),
                scope: ModuleResolutionScope::Platform,
                root_dependencies: Vec::new(),
            },
        )
        .await
        .expect_err("invalid deployment platform version must fail closed");
        assert!(matches!(
            error,
            ModuleResolutionError::InvalidPlatformVersion(version) if version == "not-semver"
        ));

        let mut catalog = Catalog::default();
        let mut candidate = candidate("invalid_platform", "1.0.0", &[]);
        candidate.platform_compatibility = "not-a-range".into();
        catalog.0.insert("invalid_platform".into(), vec![candidate]);
        let error = resolve_module_dependencies(
            &catalog,
            ModuleResolutionRequest {
                graph_revision: 4,
                runtime_abi: "v1".into(),
                platform_version: "1.2.3".into(),
                scope: ModuleResolutionScope::Platform,
                root_dependencies: vec![ModuleDependencyConstraint {
                    slug: "invalid_platform".into(),
                    version_requirement: "^1".into(),
                }],
            },
        )
        .await
        .expect_err("invalid candidate compatibility range must fail closed");
        assert!(matches!(
            error,
            ModuleResolutionError::InvalidPlatformCompatibility(range) if range == "not-a-range"
        ));
    }

    #[test]
    fn semver_ranges_cover_caret_tilde_and_partial_versions() {
        assert!(
            version_requirement_range("^0.2.3")
                .unwrap()
                .contains(&Version::new(0, 2, 9))
        );
        assert!(
            !version_requirement_range("^0.2.3")
                .unwrap()
                .contains(&Version::new(0, 3, 0))
        );
        assert!(
            version_requirement_range("~1.2")
                .unwrap()
                .contains(&Version::new(1, 2, 99))
        );
        assert!(
            !version_requirement_range("1")
                .unwrap()
                .contains(&Version::new(2, 0, 0))
        );
    }
}
