use std::collections::{BTreeMap, BTreeSet};

use rhai::{module_resolvers::StaticModuleResolver, Engine, Module, Scope};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const ALLOY_WORKSPACE_SCHEMA_VERSION: u16 = 1;
pub const MAX_WORKSPACE_FILES: usize = 64;
pub const MAX_WORKSPACE_FILE_BYTES: usize = 128 * 1024;
pub const MAX_WORKSPACE_BYTES: usize = 1024 * 1024;
pub const MAX_WORKSPACE_PATH_BYTES: usize = 160;
pub const MAX_WORKSPACE_IMPORT_DEPTH: usize = 8;

/// A bounded, data-only Alloy source workspace. It is stored and hashed as one
/// canonical value; the sandbox receives it as request bytes and never mounts
/// it into a guest-visible filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlloyWorkspace {
    pub schema_version: u16,
    pub entrypoint: String,
    pub files: Vec<WorkspaceFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceFileKind {
    Source,
    Test,
    Fixture,
    Schema,
    Policy,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub path: String,
    pub kind: WorkspaceFileKind,
    pub contents: String,
}

impl AlloyWorkspace {
    pub fn single_source(source: impl Into<String>) -> Self {
        Self {
            schema_version: ALLOY_WORKSPACE_SCHEMA_VERSION,
            entrypoint: "src/main.rhai".to_string(),
            files: vec![WorkspaceFile {
                path: "src/main.rhai".to_string(),
                kind: WorkspaceFileKind::Source,
                contents: source.into(),
            }],
        }
    }

    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if self.schema_version != ALLOY_WORKSPACE_SCHEMA_VERSION {
            return Err(WorkspaceError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if self.files.is_empty() || self.files.len() > MAX_WORKSPACE_FILES {
            return Err(WorkspaceError::InvalidFileCount {
                limit: MAX_WORKSPACE_FILES,
            });
        }
        validate_path(&self.entrypoint)?;

        let mut paths = BTreeSet::new();
        let mut total_bytes = 0usize;
        let mut entrypoint_kind = None;
        for file in &self.files {
            validate_path(&file.path)?;
            if !paths.insert(&file.path) {
                return Err(WorkspaceError::DuplicatePath(file.path.clone()));
            }
            validate_file_kind(file)?;
            let size = file.contents.len();
            if size > MAX_WORKSPACE_FILE_BYTES {
                return Err(WorkspaceError::FileTooLarge {
                    path: file.path.clone(),
                    limit: MAX_WORKSPACE_FILE_BYTES,
                });
            }
            total_bytes = total_bytes
                .checked_add(size)
                .ok_or(WorkspaceError::TooLarge {
                    limit: MAX_WORKSPACE_BYTES,
                })?;
            if total_bytes > MAX_WORKSPACE_BYTES {
                return Err(WorkspaceError::TooLarge {
                    limit: MAX_WORKSPACE_BYTES,
                });
            }
            if file.path == self.entrypoint {
                entrypoint_kind = Some(file.kind);
            }
        }

        match entrypoint_kind {
            Some(WorkspaceFileKind::Source) => Ok(()),
            Some(_) => Err(WorkspaceError::EntrypointMustBeSource),
            None => Err(WorkspaceError::MissingEntrypoint(self.entrypoint.clone())),
        }
    }

    pub fn entrypoint_source(&self) -> Result<&str, WorkspaceError> {
        self.executable_entrypoint_source(&self.entrypoint, false)
    }

    /// Returns a declared test source from this workspace. Test entrypoints are
    /// never production entrypoints and can only import bounded `src/*.rhai`
    /// workspace modules through the same in-memory resolver.
    pub fn test_source(&self, test_path: &str) -> Result<&str, WorkspaceError> {
        self.validate()?;
        let file = self
            .files
            .iter()
            .find(|file| file.path == test_path)
            .ok_or_else(|| WorkspaceError::MissingTestEntrypoint(test_path.to_string()))?;
        if file.kind != WorkspaceFileKind::Test {
            return Err(WorkspaceError::TestEntrypointMustBeTest(
                test_path.to_string(),
            ));
        }
        Ok(file.contents.as_str())
    }

    /// Returns the only source bytes that a sandbox request may execute: the
    /// declared production entrypoint or one declared test entrypoint.
    pub fn executable_source(&self, entrypoint: &str) -> Result<&str, WorkspaceError> {
        self.executable_entrypoint_source(entrypoint, true)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, WorkspaceError> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        serde_json::to_vec(&canonical).map_err(|error| WorkspaceError::Serialize(error.to_string()))
    }

    pub fn digest(&self) -> Result<String, WorkspaceError> {
        Ok(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(self.canonical_bytes()?))
        ))
    }

    /// Installs a request-private Rhai resolver backed entirely by this validated
    /// workspace. Modules are compiled in dependency order into Rhai's public
    /// static resolver; no host filesystem path or external source is available.
    pub fn configure_rhai_engine(&self, engine: &mut Engine) -> Result<(), WorkspaceError> {
        self.configure_rhai_engine_for_entrypoint(engine, &self.entrypoint)
    }

    /// Installs the request-private resolver for either the declared production
    /// entrypoint or one declared `tests/*.rhai` entrypoint. No other workspace
    /// file kind is executable.
    pub fn configure_rhai_engine_for_entrypoint(
        &self,
        engine: &mut Engine,
        entrypoint: &str,
    ) -> Result<(), WorkspaceError> {
        self.validate()?;
        let modules = self
            .files
            .iter()
            .filter(|file| file.kind == WorkspaceFileKind::Source)
            .map(|file| (file.path.clone(), file.contents.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut imports = modules
            .iter()
            .map(|(path, source)| Ok((path.clone(), parse_workspace_imports(path, source)?)))
            .collect::<Result<BTreeMap<_, _>, WorkspaceError>>()?;
        let entrypoint_source = self.executable_entrypoint_source(entrypoint, true)?;
        imports.insert(
            entrypoint.to_string(),
            parse_workspace_imports(entrypoint, entrypoint_source)?,
        );
        validate_import_graph(&imports, &modules)?;

        let mut resolver = StaticModuleResolver::new();
        let mut compiled = BTreeSet::new();
        for imported_path in imports.get(entrypoint).into_iter().flatten() {
            compile_workspace_module(
                imported_path,
                engine,
                &modules,
                &imports,
                &mut resolver,
                &mut compiled,
            )?;
        }
        engine.set_module_resolver(resolver);
        Ok(())
    }

    /// Validates the entrypoint and every reachable import through the same
    /// in-memory resolver that the sandbox request installs. This is used by
    /// authoring transports before persisting a workspace.
    pub fn validate_rhai_workspace(&self) -> Result<(), WorkspaceError> {
        self.validate_rhai_entrypoint(&self.entrypoint)
    }

    /// Validates a declared test entrypoint against the exact resolver that a
    /// sandbox test request uses.
    pub fn validate_rhai_test(&self, test_path: &str) -> Result<(), WorkspaceError> {
        self.test_source(test_path)?;
        self.validate_rhai_entrypoint(test_path)
    }

    fn validate_rhai_entrypoint(&self, entrypoint: &str) -> Result<(), WorkspaceError> {
        let mut engine = Engine::new();
        self.configure_rhai_engine_for_entrypoint(&mut engine, entrypoint)?;
        let source = self.executable_entrypoint_source(entrypoint, true)?;
        engine
            .compile_into_self_contained(&Scope::new(), source)
            .map_err(|error| WorkspaceError::ModuleCompilation {
                path: entrypoint.to_string(),
                message: error.to_string(),
            })?;
        Ok(())
    }

    fn executable_entrypoint_source(
        &self,
        entrypoint: &str,
        allow_test: bool,
    ) -> Result<&str, WorkspaceError> {
        self.validate()?;
        if entrypoint == self.entrypoint {
            return self
                .files
                .iter()
                .find(|file| file.path == entrypoint)
                .map(|file| file.contents.as_str())
                .ok_or_else(|| WorkspaceError::MissingEntrypoint(entrypoint.to_string()));
        }
        if allow_test {
            return self.test_source(entrypoint);
        }
        Err(WorkspaceError::UnsupportedExecutionEntrypoint(
            entrypoint.to_string(),
        ))
    }
}

fn compile_workspace_module(
    path: &str,
    engine: &mut Engine,
    modules: &BTreeMap<String, String>,
    imports: &BTreeMap<String, Vec<String>>,
    resolver: &mut StaticModuleResolver,
    compiled: &mut BTreeSet<String>,
) -> Result<(), WorkspaceError> {
    if !compiled.insert(path.to_string()) {
        return Ok(());
    }
    for imported_path in imports.get(path).into_iter().flatten() {
        compile_workspace_module(imported_path, engine, modules, imports, resolver, compiled)?;
    }
    engine.set_module_resolver(resolver.clone());
    let source = modules
        .get(path)
        .ok_or_else(|| WorkspaceError::MissingImportedModule(path.to_string()))?;
    let mut ast = engine
        .compile(source)
        .map_err(|error| WorkspaceError::ModuleCompilation {
            path: path.to_string(),
            message: error.to_string(),
        })?;
    ast.set_source(path);
    let module = Module::eval_ast_as_new(Scope::new(), &ast, engine).map_err(|error| {
        WorkspaceError::ModuleCompilation {
            path: path.to_string(),
            message: error.to_string(),
        }
    })?;
    resolver.insert(path, module);
    Ok(())
}

fn parse_workspace_imports(path: &str, source: &str) -> Result<Vec<String>, WorkspaceError> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            (!line.starts_with("//")).then_some(line)
        })
        .filter_map(|line| line.strip_prefix("import "))
        .map(|line| parse_workspace_import(path, line))
        .collect()
}

fn parse_workspace_import(source_path: &str, statement: &str) -> Result<String, WorkspaceError> {
    let statement = statement.trim();
    let Some(path_and_tail) = statement.strip_prefix('"') else {
        return Err(WorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "imports must use a quoted workspace path".to_string(),
        });
    };
    let Some((imported_path, tail)) = path_and_tail.split_once('"') else {
        return Err(WorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "imports must terminate the quoted workspace path".to_string(),
        });
    };
    let Some(alias) = tail.trim().strip_prefix("as ") else {
        return Err(WorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "imports must declare an alias".to_string(),
        });
    };
    let alias = alias.trim_end();
    let Some(alias) = alias.strip_suffix(';') else {
        return Err(WorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "imports must occupy one statement per line".to_string(),
        });
    };
    if alias.is_empty()
        || !alias
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(WorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "imports must use an ASCII identifier alias".to_string(),
        });
    }
    if !imported_path.starts_with("src/") || !imported_path.ends_with(".rhai") {
        return Err(WorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "workspace imports must use an exact src/*.rhai path".to_string(),
        });
    }
    validate_path(imported_path)?;
    Ok(imported_path.to_string())
}

fn validate_import_graph(
    imports: &BTreeMap<String, Vec<String>>,
    modules: &BTreeMap<String, String>,
) -> Result<(), WorkspaceError> {
    for imported_paths in imports.values() {
        for imported_path in imported_paths {
            if !modules.contains_key(imported_path) {
                return Err(WorkspaceError::MissingImportedModule(imported_path.clone()));
            }
        }
    }
    let mut visiting = Vec::new();
    let mut visited = BTreeSet::new();
    for path in imports.keys() {
        validate_import_depth(path, imports, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn validate_import_depth(
    path: &str,
    imports: &BTreeMap<String, Vec<String>>,
    visiting: &mut Vec<String>,
    visited: &mut BTreeSet<String>,
) -> Result<(), WorkspaceError> {
    if visited.contains(path) {
        return Ok(());
    }
    if let Some(index) = visiting.iter().position(|current| current == path) {
        let mut cycle = visiting[index..].to_vec();
        cycle.push(path.to_string());
        return Err(WorkspaceError::ImportCycle(cycle.join(" -> ")));
    }
    if visiting.len() >= MAX_WORKSPACE_IMPORT_DEPTH {
        return Err(WorkspaceError::ImportDepthExceeded {
            limit: MAX_WORKSPACE_IMPORT_DEPTH,
        });
    }
    visiting.push(path.to_string());
    for imported_path in imports.get(path).into_iter().flatten() {
        validate_import_depth(imported_path, imports, visiting, visited)?;
    }
    visiting.pop();
    visited.insert(path.to_string());
    Ok(())
}

fn validate_path(path: &str) -> Result<(), WorkspaceError> {
    if path.is_empty()
        || path.len() > MAX_WORKSPACE_PATH_BYTES
        || path.starts_with('/')
        || path.contains('\\')
    {
        return Err(WorkspaceError::InvalidPath(path.to_string()));
    }
    for segment in path.split('/') {
        if segment.is_empty()
            || matches!(segment, "." | "..")
            || !segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(WorkspaceError::InvalidPath(path.to_string()));
        }
    }
    Ok(())
}

fn validate_file_kind(file: &WorkspaceFile) -> Result<(), WorkspaceError> {
    let correct_root = match file.kind {
        WorkspaceFileKind::Source => file.path.starts_with("src/") && file.path.ends_with(".rhai"),
        WorkspaceFileKind::Test => file.path.starts_with("tests/") && file.path.ends_with(".rhai"),
        WorkspaceFileKind::Fixture => file.path.starts_with("fixtures/"),
        WorkspaceFileKind::Schema => file.path.starts_with("schemas/"),
        WorkspaceFileKind::Policy => file.path.starts_with("policy/"),
        WorkspaceFileKind::Generated => file.path.starts_with("generated/"),
    };
    if !correct_root {
        return Err(WorkspaceError::InvalidFileLocation {
            path: file.path.clone(),
            kind: file.kind,
        });
    }
    if matches!(
        file.kind,
        WorkspaceFileKind::Source | WorkspaceFileKind::Test
    ) && file.contents.trim().is_empty()
    {
        return Err(WorkspaceError::EmptyRhaiFile(file.path.clone()));
    }
    Ok(())
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WorkspaceError {
    #[error("unsupported Alloy workspace schema version {0}")]
    UnsupportedSchemaVersion(u16),
    #[error("Alloy workspace must contain between one and {limit} files")]
    InvalidFileCount { limit: usize },
    #[error("invalid Alloy workspace path `{0}`")]
    InvalidPath(String),
    #[error("Alloy workspace contains duplicate path `{0}`")]
    DuplicatePath(String),
    #[error("Alloy workspace file `{path}` exceeds {limit} bytes")]
    FileTooLarge { path: String, limit: usize },
    #[error("Alloy workspace exceeds {limit} bytes")]
    TooLarge { limit: usize },
    #[error("Alloy workspace entrypoint `{0}` does not exist")]
    MissingEntrypoint(String),
    #[error("Alloy workspace entrypoint must be a source file")]
    EntrypointMustBeSource,
    #[error("Alloy workspace test entrypoint `{0}` does not exist")]
    MissingTestEntrypoint(String),
    #[error("Alloy workspace test entrypoint `{0}` must be a test file")]
    TestEntrypointMustBeTest(String),
    #[error("Alloy workspace file `{0}` is not an executable entrypoint")]
    UnsupportedExecutionEntrypoint(String),
    #[error("Alloy workspace file `{path}` is not valid for kind `{kind:?}`")]
    InvalidFileLocation {
        path: String,
        kind: WorkspaceFileKind,
    },
    #[error("Alloy workspace import in `{source_path}` is invalid: {message}")]
    InvalidImport {
        source_path: String,
        message: String,
    },
    #[error("Alloy workspace import references missing source module `{0}`")]
    MissingImportedModule(String),
    #[error("Alloy workspace import cycle: {0}")]
    ImportCycle(String),
    #[error("Alloy workspace import depth exceeds {limit} modules")]
    ImportDepthExceeded { limit: usize },
    #[error("Alloy workspace source module `{path}` failed to compile: {message}")]
    ModuleCompilation { path: String, message: String },
    #[error("Alloy Rhai file `{0}` must not be empty")]
    EmptyRhaiFile(String),
    #[error("Alloy workspace serialization failed: {0}")]
    Serialize(String),
}

#[cfg(test)]
mod tests {
    use rhai::Engine;

    use super::{
        AlloyWorkspace, WorkspaceError, WorkspaceFile, WorkspaceFileKind,
        MAX_WORKSPACE_IMPORT_DEPTH,
    };

    #[test]
    fn canonical_workspace_digest_is_independent_of_file_order() {
        let mut first = AlloyWorkspace::single_source("42");
        first.files.push(WorkspaceFile {
            path: "fixtures/input.json".into(),
            kind: WorkspaceFileKind::Fixture,
            contents: "{}".into(),
        });
        let mut second = first.clone();
        second.files.reverse();

        assert_eq!(first.digest(), second.digest());
    }

    #[test]
    fn workspace_rejects_escape_paths_and_invalid_entrypoints() {
        let mut workspace = AlloyWorkspace::single_source("42");
        workspace.entrypoint = "../main.rhai".into();
        assert!(matches!(
            workspace.validate(),
            Err(WorkspaceError::InvalidPath(_))
        ));

        let mut workspace = AlloyWorkspace::single_source("42");
        workspace.entrypoint = "fixtures/input.json".into();
        assert!(matches!(
            workspace.validate(),
            Err(WorkspaceError::MissingEntrypoint(_))
        ));
    }

    #[test]
    fn workspace_resolver_loads_only_in_memory_source_modules() {
        let workspace = AlloyWorkspace {
            schema_version: 1,
            entrypoint: "src/main.rhai".into(),
            files: vec![
                WorkspaceFile {
                    path: "src/main.rhai".into(),
                    kind: WorkspaceFileKind::Source,
                    contents: "import \"src/math.rhai\" as math;\nmath::double(21)".into(),
                },
                WorkspaceFile {
                    path: "src/math.rhai".into(),
                    kind: WorkspaceFileKind::Source,
                    contents: "fn double(value) { value * 2 }".into(),
                },
            ],
        };
        let mut engine = Engine::new();
        workspace
            .configure_rhai_engine(&mut engine)
            .expect("resolver");
        workspace
            .validate_rhai_workspace()
            .expect("workspace should validate");

        assert_eq!(
            engine
                .eval::<i64>(workspace.entrypoint_source().expect("entrypoint"))
                .expect("workspace import should resolve"),
            42
        );
    }

    #[test]
    fn workspace_resolver_rejects_import_cycles_and_depth_overflow() {
        let mut files = vec![WorkspaceFile {
            path: "src/main.rhai".into(),
            kind: WorkspaceFileKind::Source,
            contents: "import \"src/module_0.rhai\" as module_0; 1".into(),
        }];
        for index in 0..=MAX_WORKSPACE_IMPORT_DEPTH {
            let next = if index == MAX_WORKSPACE_IMPORT_DEPTH {
                "src/module_0.rhai".to_string()
            } else {
                format!("src/module_{}.rhai", index + 1)
            };
            files.push(WorkspaceFile {
                path: format!("src/module_{index}.rhai"),
                kind: WorkspaceFileKind::Source,
                contents: format!("import \"{next}\" as next; fn value() {{ 1 }}"),
            });
        }
        let workspace = AlloyWorkspace {
            schema_version: 1,
            entrypoint: "src/main.rhai".into(),
            files,
        };
        let mut engine = Engine::new();
        let error = workspace
            .configure_rhai_engine(&mut engine)
            .expect_err("cyclic/deep import graph must fail");
        assert!(error.to_string().contains("workspace import"));
    }

    #[test]
    fn test_entrypoint_uses_the_workspace_resolver_without_becoming_production_source() {
        let workspace = AlloyWorkspace {
            schema_version: 1,
            entrypoint: "src/main.rhai".into(),
            files: vec![
                WorkspaceFile {
                    path: "src/main.rhai".into(),
                    kind: WorkspaceFileKind::Source,
                    contents: "fn live() { false }".into(),
                },
                WorkspaceFile {
                    path: "src/assertions.rhai".into(),
                    kind: WorkspaceFileKind::Source,
                    contents: "fn equals(left, right) { left == right }".into(),
                },
                WorkspaceFile {
                    path: "tests/live.rhai".into(),
                    kind: WorkspaceFileKind::Test,
                    contents: "import \"src/assertions.rhai\" as assertions;\nassertions::equals(21 * 2, 42)".into(),
                },
            ],
        };
        let mut engine = Engine::new();
        workspace
            .configure_rhai_engine_for_entrypoint(&mut engine, "tests/live.rhai")
            .expect("test resolver");
        workspace
            .validate_rhai_test("tests/live.rhai")
            .expect("test should validate");

        assert!(engine
            .eval::<bool>(
                workspace
                    .test_source("tests/live.rhai")
                    .expect("test source")
            )
            .expect("test import should resolve"));
        assert!(matches!(
            workspace.test_source("src/main.rhai"),
            Err(WorkspaceError::TestEntrypointMustBeTest(_))
        ));
    }
}
