//! Canonical bounded Rhai source workspace shared by drafts and artifacts.

use std::collections::BTreeSet;

#[cfg(feature = "rhai")]
use rhai::{Engine, Module, Scope, module_resolvers::StaticModuleResolver};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(feature = "rhai")]
use std::collections::BTreeMap;
use thiserror::Error;

use crate::capability::CapabilityName;

pub const RHAI_WORKSPACE_MEDIA_TYPE: &str = "application/vnd.rustok.rhai.workspace.v1";
pub const RHAI_WORKSPACE_SCHEMA_VERSION: u16 = 1;
pub const MAX_RHAI_WORKSPACE_FILES: usize = 64;
pub const MAX_RHAI_WORKSPACE_FILE_BYTES: usize = 128 * 1024;
pub const MAX_RHAI_WORKSPACE_BYTES: usize = 1024 * 1024;
pub const MAX_RHAI_WORKSPACE_PATH_BYTES: usize = 160;
pub const MAX_RHAI_WORKSPACE_IMPORT_DEPTH: usize = 8;

/// A bounded, data-only Rhai source workspace. It is stored and hashed as one
/// canonical value; the sandbox receives it as request bytes and never mounts
/// it into a guest-visible filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RhaiWorkspace {
    pub schema_version: u16,
    pub entrypoint: String,
    pub files: Vec<RhaiWorkspaceFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RhaiWorkspaceFileKind {
    Source,
    Test,
    Fixture,
    Schema,
    Policy,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RhaiWorkspaceFile {
    pub path: String,
    pub kind: RhaiWorkspaceFileKind,
    pub contents: String,
}

impl RhaiWorkspace {
    pub fn single_source(source: impl Into<String>) -> Self {
        Self {
            schema_version: RHAI_WORKSPACE_SCHEMA_VERSION,
            entrypoint: "src/main.rhai".to_string(),
            files: vec![RhaiWorkspaceFile {
                path: "src/main.rhai".to_string(),
                kind: RhaiWorkspaceFileKind::Source,
                contents: source.into(),
            }],
        }
    }

    pub fn validate(&self) -> Result<(), RhaiWorkspaceError> {
        if self.schema_version != RHAI_WORKSPACE_SCHEMA_VERSION {
            return Err(RhaiWorkspaceError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if self.files.is_empty() || self.files.len() > MAX_RHAI_WORKSPACE_FILES {
            return Err(RhaiWorkspaceError::InvalidFileCount {
                limit: MAX_RHAI_WORKSPACE_FILES,
            });
        }
        validate_path(&self.entrypoint)?;

        let mut paths = BTreeSet::new();
        let mut total_bytes = 0usize;
        let mut entrypoint_kind = None;
        for file in &self.files {
            validate_path(&file.path)?;
            if !paths.insert(&file.path) {
                return Err(RhaiWorkspaceError::DuplicatePath(file.path.clone()));
            }
            validate_file_kind(file)?;
            let size = file.contents.len();
            if size > MAX_RHAI_WORKSPACE_FILE_BYTES {
                return Err(RhaiWorkspaceError::FileTooLarge {
                    path: file.path.clone(),
                    limit: MAX_RHAI_WORKSPACE_FILE_BYTES,
                });
            }
            total_bytes = total_bytes
                .checked_add(size)
                .ok_or(RhaiWorkspaceError::TooLarge {
                    limit: MAX_RHAI_WORKSPACE_BYTES,
                })?;
            if total_bytes > MAX_RHAI_WORKSPACE_BYTES {
                return Err(RhaiWorkspaceError::TooLarge {
                    limit: MAX_RHAI_WORKSPACE_BYTES,
                });
            }
            if file.path == self.entrypoint {
                entrypoint_kind = Some(file.kind);
            }
        }

        match entrypoint_kind {
            Some(RhaiWorkspaceFileKind::Source) => Ok(()),
            Some(_) => Err(RhaiWorkspaceError::EntrypointMustBeSource),
            None => Err(RhaiWorkspaceError::MissingEntrypoint(
                self.entrypoint.clone(),
            )),
        }
    }

    pub fn entrypoint_source(&self) -> Result<&str, RhaiWorkspaceError> {
        self.executable_entrypoint_source(&self.entrypoint, false)
    }

    /// Returns a declared test source from this workspace. Test entrypoints are
    /// never production entrypoints and can only import bounded `src/*.rhai`
    /// workspace modules through the same in-memory resolver.
    pub fn test_source(&self, test_path: &str) -> Result<&str, RhaiWorkspaceError> {
        self.validate()?;
        let file = self
            .files
            .iter()
            .find(|file| file.path == test_path)
            .ok_or_else(|| RhaiWorkspaceError::MissingTestEntrypoint(test_path.to_string()))?;
        if file.kind != RhaiWorkspaceFileKind::Test {
            return Err(RhaiWorkspaceError::TestEntrypointMustBeTest(
                test_path.to_string(),
            ));
        }
        Ok(file.contents.as_str())
    }

    /// Returns the only source bytes that a sandbox request may execute: the
    /// declared production entrypoint or one declared test entrypoint.
    pub fn executable_source(&self, entrypoint: &str) -> Result<&str, RhaiWorkspaceError> {
        self.executable_entrypoint_source(entrypoint, true)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RhaiWorkspaceError> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        serde_json::to_vec(&canonical)
            .map_err(|error| RhaiWorkspaceError::Serialize(error.to_string()))
    }

    pub fn digest(&self) -> Result<String, RhaiWorkspaceError> {
        Ok(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(self.canonical_bytes()?))
        ))
    }

    /// Returns the exact capability names reachable from executable Rhai
    /// source files through the neutral sandbox helper surface. Publication
    /// accepts only literal generic capability names: a dynamically chosen
    /// capability cannot be proven against an immutable descriptor.
    pub fn observed_capabilities(
        &self,
    ) -> Result<Vec<CapabilityName>, RhaiWorkspaceCapabilityError> {
        self.validate()
            .map_err(RhaiWorkspaceCapabilityError::Workspace)?;

        let mut observed = BTreeSet::new();
        for file in self
            .files
            .iter()
            .filter(|file| file.kind == RhaiWorkspaceFileKind::Source)
        {
            observe_rhai_source_capabilities(&file.path, &file.contents, &mut observed)?;
        }

        observed
            .into_iter()
            .map(|name| {
                CapabilityName::new(name).map_err(|_| {
                    RhaiWorkspaceCapabilityError::InvalidLiteralCapability {
                        path: self.entrypoint.clone(),
                    }
                })
            })
            .collect()
    }

    /// Requires the declared descriptor capability set to be exactly the set
    /// of capability helpers observed in immutable source. This prevents both
    /// undeclared tool calls and broader unused declarations from reaching
    /// admission, where tenant policy could otherwise grant them later.
    pub fn validate_declared_capabilities(
        &self,
        declared_capabilities: &[CapabilityName],
    ) -> Result<(), RhaiWorkspaceCapabilityError> {
        let observed = self
            .observed_capabilities()?
            .into_iter()
            .map(|capability| capability.as_str().to_string())
            .collect::<BTreeSet<_>>();
        let declared = declared_capabilities
            .iter()
            .map(|capability| capability.as_str().to_string())
            .collect::<BTreeSet<_>>();
        let missing = observed.difference(&declared).cloned().collect::<Vec<_>>();
        let unused = declared.difference(&observed).cloned().collect::<Vec<_>>();
        if missing.is_empty() && unused.is_empty() {
            Ok(())
        } else {
            Err(RhaiWorkspaceCapabilityError::CapabilityDeclarationMismatch { missing, unused })
        }
    }

    /// Installs a request-private Rhai resolver backed entirely by this validated
    /// workspace. Modules are compiled in dependency order into Rhai's public
    /// static resolver; no host filesystem path or external source is available.
    #[cfg(feature = "rhai")]
    pub fn configure_rhai_engine(&self, engine: &mut Engine) -> Result<(), RhaiWorkspaceError> {
        self.configure_rhai_engine_for_entrypoint(engine, &self.entrypoint)
    }

    /// Installs the request-private resolver for either the declared production
    /// entrypoint or one declared `tests/*.rhai` entrypoint. No other workspace
    /// file kind is executable.
    #[cfg(feature = "rhai")]
    pub fn configure_rhai_engine_for_entrypoint(
        &self,
        engine: &mut Engine,
        entrypoint: &str,
    ) -> Result<(), RhaiWorkspaceError> {
        self.validate()?;
        let modules = self
            .files
            .iter()
            .filter(|file| file.kind == RhaiWorkspaceFileKind::Source)
            .map(|file| (file.path.clone(), file.contents.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut imports = modules
            .iter()
            .map(|(path, source)| Ok((path.clone(), parse_workspace_imports(path, source)?)))
            .collect::<Result<BTreeMap<_, _>, RhaiWorkspaceError>>()?;
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
    #[cfg(feature = "rhai")]
    pub fn validate_rhai_workspace(&self) -> Result<(), RhaiWorkspaceError> {
        self.validate_rhai_entrypoint(&self.entrypoint)
    }

    /// Validates a declared test entrypoint against the exact resolver that a
    /// sandbox test request uses.
    #[cfg(feature = "rhai")]
    pub fn validate_rhai_test(&self, test_path: &str) -> Result<(), RhaiWorkspaceError> {
        self.test_source(test_path)?;
        self.validate_rhai_entrypoint(test_path)
    }

    #[cfg(feature = "rhai")]
    fn validate_rhai_entrypoint(&self, entrypoint: &str) -> Result<(), RhaiWorkspaceError> {
        let mut engine = Engine::new();
        self.configure_rhai_engine_for_entrypoint(&mut engine, entrypoint)?;
        let source = self.executable_entrypoint_source(entrypoint, true)?;
        engine
            .compile_into_self_contained(&Scope::new(), source)
            .map_err(|error| RhaiWorkspaceError::ModuleCompilation {
                path: entrypoint.to_string(),
                message: error.to_string(),
            })?;
        Ok(())
    }

    fn executable_entrypoint_source(
        &self,
        entrypoint: &str,
        allow_test: bool,
    ) -> Result<&str, RhaiWorkspaceError> {
        self.validate()?;
        if entrypoint == self.entrypoint {
            return self
                .files
                .iter()
                .find(|file| file.path == entrypoint)
                .map(|file| file.contents.as_str())
                .ok_or_else(|| RhaiWorkspaceError::MissingEntrypoint(entrypoint.to_string()));
        }
        if allow_test {
            return self.test_source(entrypoint);
        }
        Err(RhaiWorkspaceError::UnsupportedExecutionEntrypoint(
            entrypoint.to_string(),
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RhaiToken {
    Identifier(String),
    StringLiteral { value: String, escaped: bool },
    Symbol(char),
}

fn observe_rhai_source_capabilities(
    path: &str,
    source: &str,
    observed: &mut BTreeSet<String>,
) -> Result<(), RhaiWorkspaceCapabilityError> {
    let tokens = tokenize_rhai(source);
    for (index, token) in tokens.iter().enumerate() {
        let RhaiToken::Identifier(name) = token else {
            continue;
        };
        if name == "fn" {
            let reserved = match tokens.get(index + 1) {
                Some(RhaiToken::Identifier(helper)) if is_capability_helper(helper) => {
                    Some(helper.clone())
                }
                _ => None,
            };
            if let Some(helper) = reserved {
                return Err(RhaiWorkspaceCapabilityError::ReservedCapabilityHelper {
                    path: path.to_string(),
                    helper,
                });
            }
        }
        if !matches!(tokens.get(index + 1), Some(RhaiToken::Symbol('('))) {
            continue;
        }
        match name.as_str() {
            "http_get" | "http_post" | "http_request" => {
                observed.insert("platform.http".to_string());
            }
            "capability_call" => {
                let Some(RhaiToken::StringLiteral { value, escaped }) = tokens.get(index + 2)
                else {
                    return Err(RhaiWorkspaceCapabilityError::DynamicCapabilityCall {
                        path: path.to_string(),
                    });
                };
                if *escaped || CapabilityName::new(value.clone()).is_err() {
                    return Err(RhaiWorkspaceCapabilityError::InvalidLiteralCapability {
                        path: path.to_string(),
                    });
                }
                observed.insert(value.clone());
            }
            _ => {}
        }
    }
    Ok(())
}

fn is_capability_helper(name: &str) -> bool {
    matches!(
        name,
        "capability_call" | "http_get" | "http_post" | "http_request"
    )
}

fn tokenize_rhai(source: &str) -> Vec<RhaiToken> {
    let characters = source.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character.is_whitespace() {
            index += 1;
        } else if character == '/' && characters.get(index + 1) == Some(&'/') {
            index += 2;
            while index < characters.len() && characters[index] != '\n' {
                index += 1;
            }
        } else if character == '/' && characters.get(index + 1) == Some(&'*') {
            index += 2;
            let mut depth = 1_u32;
            while index < characters.len() && depth > 0 {
                if characters[index] == '/' && characters.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if characters[index] == '*' && characters.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
        } else if character == '"' {
            index += 1;
            let mut value = String::new();
            let mut escaped = false;
            while index < characters.len() {
                match characters[index] {
                    '"' => {
                        index += 1;
                        break;
                    }
                    '\\' => {
                        escaped = true;
                        index += 1;
                        if index < characters.len() {
                            value.push(characters[index]);
                            index += 1;
                        }
                    }
                    value_character => {
                        value.push(value_character);
                        index += 1;
                    }
                }
            }
            tokens.push(RhaiToken::StringLiteral { value, escaped });
        } else if character.is_ascii_alphabetic() || character == '_' {
            let start = index;
            index += 1;
            while index < characters.len()
                && (characters[index].is_ascii_alphanumeric() || characters[index] == '_')
            {
                index += 1;
            }
            tokens.push(RhaiToken::Identifier(
                characters[start..index].iter().collect(),
            ));
        } else {
            tokens.push(RhaiToken::Symbol(character));
            index += 1;
        }
    }
    tokens
}

#[cfg(feature = "rhai")]
fn compile_workspace_module(
    path: &str,
    engine: &mut Engine,
    modules: &BTreeMap<String, String>,
    imports: &BTreeMap<String, Vec<String>>,
    resolver: &mut StaticModuleResolver,
    compiled: &mut BTreeSet<String>,
) -> Result<(), RhaiWorkspaceError> {
    if !compiled.insert(path.to_string()) {
        return Ok(());
    }
    for imported_path in imports.get(path).into_iter().flatten() {
        compile_workspace_module(imported_path, engine, modules, imports, resolver, compiled)?;
    }
    engine.set_module_resolver(resolver.clone());
    let source = modules
        .get(path)
        .ok_or_else(|| RhaiWorkspaceError::MissingImportedModule(path.to_string()))?;
    let mut ast =
        engine
            .compile(source)
            .map_err(|error| RhaiWorkspaceError::ModuleCompilation {
                path: path.to_string(),
                message: error.to_string(),
            })?;
    ast.set_source(path);
    let module = Module::eval_ast_as_new(Scope::new(), &ast, engine).map_err(|error| {
        RhaiWorkspaceError::ModuleCompilation {
            path: path.to_string(),
            message: error.to_string(),
        }
    })?;
    resolver.insert(path, module);
    Ok(())
}

#[cfg(feature = "rhai")]
fn parse_workspace_imports(path: &str, source: &str) -> Result<Vec<String>, RhaiWorkspaceError> {
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

#[cfg(feature = "rhai")]
fn parse_workspace_import(
    source_path: &str,
    statement: &str,
) -> Result<String, RhaiWorkspaceError> {
    let statement = statement.trim();
    let Some(path_and_tail) = statement.strip_prefix('"') else {
        return Err(RhaiWorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "imports must use a quoted workspace path".to_string(),
        });
    };
    let Some((imported_path, tail)) = path_and_tail.split_once('"') else {
        return Err(RhaiWorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "imports must terminate the quoted workspace path".to_string(),
        });
    };
    let Some(alias) = tail.trim().strip_prefix("as ") else {
        return Err(RhaiWorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "imports must declare an alias".to_string(),
        });
    };
    let alias = alias.trim_end();
    let Some(alias) = alias.strip_suffix(';') else {
        return Err(RhaiWorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "imports must occupy one statement per line".to_string(),
        });
    };
    if alias.is_empty()
        || !alias
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(RhaiWorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "imports must use an ASCII identifier alias".to_string(),
        });
    }
    if !imported_path.starts_with("src/") || !imported_path.ends_with(".rhai") {
        return Err(RhaiWorkspaceError::InvalidImport {
            source_path: source_path.to_string(),
            message: "workspace imports must use an exact src/*.rhai path".to_string(),
        });
    }
    validate_path(imported_path)?;
    Ok(imported_path.to_string())
}

#[cfg(feature = "rhai")]
fn validate_import_graph(
    imports: &BTreeMap<String, Vec<String>>,
    modules: &BTreeMap<String, String>,
) -> Result<(), RhaiWorkspaceError> {
    for imported_paths in imports.values() {
        for imported_path in imported_paths {
            if !modules.contains_key(imported_path) {
                return Err(RhaiWorkspaceError::MissingImportedModule(
                    imported_path.clone(),
                ));
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

#[cfg(feature = "rhai")]
fn validate_import_depth(
    path: &str,
    imports: &BTreeMap<String, Vec<String>>,
    visiting: &mut Vec<String>,
    visited: &mut BTreeSet<String>,
) -> Result<(), RhaiWorkspaceError> {
    if visited.contains(path) {
        return Ok(());
    }
    if let Some(index) = visiting.iter().position(|current| current == path) {
        let mut cycle = visiting[index..].to_vec();
        cycle.push(path.to_string());
        return Err(RhaiWorkspaceError::ImportCycle(cycle.join(" -> ")));
    }
    if visiting.len() >= MAX_RHAI_WORKSPACE_IMPORT_DEPTH {
        return Err(RhaiWorkspaceError::ImportDepthExceeded {
            limit: MAX_RHAI_WORKSPACE_IMPORT_DEPTH,
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

fn validate_path(path: &str) -> Result<(), RhaiWorkspaceError> {
    if path.is_empty()
        || path.len() > MAX_RHAI_WORKSPACE_PATH_BYTES
        || path.starts_with('/')
        || path.contains('\\')
    {
        return Err(RhaiWorkspaceError::InvalidPath(path.to_string()));
    }
    for segment in path.split('/') {
        if segment.is_empty()
            || matches!(segment, "." | "..")
            || !segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(RhaiWorkspaceError::InvalidPath(path.to_string()));
        }
    }
    Ok(())
}

fn validate_file_kind(file: &RhaiWorkspaceFile) -> Result<(), RhaiWorkspaceError> {
    let correct_root = match file.kind {
        RhaiWorkspaceFileKind::Source => {
            file.path.starts_with("src/") && file.path.ends_with(".rhai")
        }
        RhaiWorkspaceFileKind::Test => {
            file.path.starts_with("tests/") && file.path.ends_with(".rhai")
        }
        RhaiWorkspaceFileKind::Fixture => file.path.starts_with("fixtures/"),
        RhaiWorkspaceFileKind::Schema => file.path.starts_with("schemas/"),
        RhaiWorkspaceFileKind::Policy => file.path.starts_with("policy/"),
        RhaiWorkspaceFileKind::Generated => file.path.starts_with("generated/"),
    };
    if !correct_root {
        return Err(RhaiWorkspaceError::InvalidFileLocation {
            path: file.path.clone(),
            kind: file.kind,
        });
    }
    if matches!(
        file.kind,
        RhaiWorkspaceFileKind::Source | RhaiWorkspaceFileKind::Test
    ) && file.contents.trim().is_empty()
    {
        return Err(RhaiWorkspaceError::EmptyRhaiFile(file.path.clone()));
    }
    Ok(())
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RhaiWorkspaceError {
    #[error("unsupported Rhai workspace schema version {0}")]
    UnsupportedSchemaVersion(u16),
    #[error("Rhai workspace must contain between one and {limit} files")]
    InvalidFileCount { limit: usize },
    #[error("invalid Rhai workspace path `{0}`")]
    InvalidPath(String),
    #[error("Rhai workspace contains duplicate path `{0}`")]
    DuplicatePath(String),
    #[error("Rhai workspace file `{path}` exceeds {limit} bytes")]
    FileTooLarge { path: String, limit: usize },
    #[error("Rhai workspace exceeds {limit} bytes")]
    TooLarge { limit: usize },
    #[error("Rhai workspace entrypoint `{0}` does not exist")]
    MissingEntrypoint(String),
    #[error("Rhai workspace entrypoint must be a source file")]
    EntrypointMustBeSource,
    #[error("Rhai workspace test entrypoint `{0}` does not exist")]
    MissingTestEntrypoint(String),
    #[error("Rhai workspace test entrypoint `{0}` must be a test file")]
    TestEntrypointMustBeTest(String),
    #[error("Rhai workspace file `{0}` is not an executable entrypoint")]
    UnsupportedExecutionEntrypoint(String),
    #[error("Rhai workspace file `{path}` is not valid for kind `{kind:?}`")]
    InvalidFileLocation {
        path: String,
        kind: RhaiWorkspaceFileKind,
    },
    #[error("Rhai workspace import in `{source_path}` is invalid: {message}")]
    InvalidImport {
        source_path: String,
        message: String,
    },
    #[error("Rhai workspace import references missing source module `{0}`")]
    MissingImportedModule(String),
    #[error("Rhai workspace import cycle: {0}")]
    ImportCycle(String),
    #[error("Rhai workspace import depth exceeds {limit} modules")]
    ImportDepthExceeded { limit: usize },
    #[error("Rhai workspace source module `{path}` failed to compile: {message}")]
    ModuleCompilation { path: String, message: String },
    #[error("Rhai workspace file `{0}` must not be empty")]
    EmptyRhaiFile(String),
    #[error("Rhai workspace serialization failed: {0}")]
    Serialize(String),
}

/// Publication-time capability evidence derived from the exact immutable Rhai
/// workspace. The sandbox owns this because both Alloy authoring and isolated
/// artifact validation must apply the same neutral helper semantics.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RhaiWorkspaceCapabilityError {
    #[error("Rhai workspace is invalid: {0}")]
    Workspace(#[source] RhaiWorkspaceError),
    #[error(
        "Rhai source uses a dynamic capability name in `{path}`; publication requires a literal declared name"
    )]
    DynamicCapabilityCall { path: String },
    #[error("Rhai source redefines reserved capability helper `{helper}` in `{path}")]
    ReservedCapabilityHelper { path: String, helper: String },
    #[error(
        "Rhai capability declarations do not match source tool use; missing declarations: {missing:?}; unused declarations: {unused:?}"
    )]
    CapabilityDeclarationMismatch {
        missing: Vec<String>,
        unused: Vec<String>,
    },
    #[error("Rhai source contains an invalid literal capability in `{path}")]
    InvalidLiteralCapability { path: String },
}

#[cfg(all(test, feature = "rhai"))]
mod tests {
    use rhai::Engine;

    use super::{
        MAX_RHAI_WORKSPACE_IMPORT_DEPTH, RhaiWorkspace, RhaiWorkspaceError, RhaiWorkspaceFile,
        RhaiWorkspaceFileKind,
    };
    use crate::CapabilityName;

    #[test]
    fn canonical_workspace_digest_is_independent_of_file_order() {
        let mut first = RhaiWorkspace::single_source("42");
        first.files.push(RhaiWorkspaceFile {
            path: "fixtures/input.json".into(),
            kind: RhaiWorkspaceFileKind::Fixture,
            contents: "{}".into(),
        });
        let mut second = first.clone();
        second.files.reverse();

        assert_eq!(first.digest(), second.digest());
    }

    #[test]
    fn workspace_rejects_escape_paths_and_invalid_entrypoints() {
        let mut workspace = RhaiWorkspace::single_source("42");
        workspace.entrypoint = "../main.rhai".into();
        assert!(matches!(
            workspace.validate(),
            Err(RhaiWorkspaceError::InvalidPath(_))
        ));

        let mut workspace = RhaiWorkspace::single_source("42");
        workspace.entrypoint = "fixtures/input.json".into();
        assert!(matches!(
            workspace.validate(),
            Err(RhaiWorkspaceError::MissingEntrypoint(_))
        ));
    }

    #[test]
    fn workspace_resolver_loads_only_in_memory_source_modules() {
        let workspace = RhaiWorkspace {
            schema_version: 1,
            entrypoint: "src/main.rhai".into(),
            files: vec![
                RhaiWorkspaceFile {
                    path: "src/main.rhai".into(),
                    kind: RhaiWorkspaceFileKind::Source,
                    contents: "import \"src/math.rhai\" as math;\nmath::double(21)".into(),
                },
                RhaiWorkspaceFile {
                    path: "src/math.rhai".into(),
                    kind: RhaiWorkspaceFileKind::Source,
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
        let mut files = vec![RhaiWorkspaceFile {
            path: "src/main.rhai".into(),
            kind: RhaiWorkspaceFileKind::Source,
            contents: "import \"src/module_0.rhai\" as module_0; 1".into(),
        }];
        for index in 0..=MAX_RHAI_WORKSPACE_IMPORT_DEPTH {
            let next = if index == MAX_RHAI_WORKSPACE_IMPORT_DEPTH {
                "src/module_0.rhai".to_string()
            } else {
                format!("src/module_{}.rhai", index + 1)
            };
            files.push(RhaiWorkspaceFile {
                path: format!("src/module_{index}.rhai"),
                kind: RhaiWorkspaceFileKind::Source,
                contents: format!("import \"{next}\" as next; fn value() {{ 1 }}"),
            });
        }
        let workspace = RhaiWorkspace {
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
        let workspace = RhaiWorkspace {
            schema_version: 1,
            entrypoint: "src/main.rhai".into(),
            files: vec![
                RhaiWorkspaceFile {
                    path: "src/main.rhai".into(),
                    kind: RhaiWorkspaceFileKind::Source,
                    contents: "fn live() { false }".into(),
                },
                RhaiWorkspaceFile {
                    path: "src/assertions.rhai".into(),
                    kind: RhaiWorkspaceFileKind::Source,
                    contents: "fn equals(left, right) { left == right }".into(),
                },
                RhaiWorkspaceFile {
                    path: "tests/live.rhai".into(),
                    kind: RhaiWorkspaceFileKind::Test,
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

        assert!(
            engine
                .eval::<bool>(
                    workspace
                        .test_source("tests/live.rhai")
                        .expect("test source")
                )
                .expect("test import should resolve")
        );
        assert!(matches!(
            workspace.test_source("src/main.rhai"),
            Err(RhaiWorkspaceError::TestEntrypointMustBeTest(_))
        ));
    }

    #[test]
    fn publication_capability_evidence_matches_only_literal_helper_calls() {
        let workspace = RhaiWorkspace {
            schema_version: 1,
            entrypoint: "src/main.rhai".into(),
            files: vec![RhaiWorkspaceFile {
                path: "src/main.rhai".into(),
                kind: RhaiWorkspaceFileKind::Source,
                contents: r#"
                        let documentation = "http_get is only documentation";
                        // capability_call("platform.secrets", "resolve", #{});
                        /* http_post("https://example.test/ignored", #{}); */
                        http_get("https://example.test/health");
                        capability_call("platform.events", "emit", #{});
                    "#
                .into(),
            }],
        };

        let observed = workspace
            .observed_capabilities()
            .expect("observe source capabilities")
            .into_iter()
            .map(|capability| capability.as_str().to_string())
            .collect::<Vec<_>>();
        assert_eq!(observed, vec!["platform.events", "platform.http"]);

        let declared = vec![
            CapabilityName::new("platform.events").expect("event capability"),
            CapabilityName::new("platform.http").expect("HTTP capability"),
        ];
        assert!(workspace.validate_declared_capabilities(&declared).is_ok());

        let error = workspace
            .validate_declared_capabilities(&[
                CapabilityName::new("platform.events").expect("event capability")
            ])
            .expect_err("missing HTTP declaration");
        assert!(matches!(
            error,
            super::RhaiWorkspaceCapabilityError::CapabilityDeclarationMismatch { missing, unused }
                if missing == vec!["platform.http"] && unused.is_empty()
        ));
    }

    #[test]
    fn publication_capability_evidence_rejects_dynamic_and_shadowed_helpers() {
        let dynamic = RhaiWorkspace::single_source(
            r#"
                let capability = "platform.events";
                capability_call(capability, "emit", #{});
            "#,
        );
        assert!(matches!(
            dynamic.validate_declared_capabilities(&[]),
            Err(super::RhaiWorkspaceCapabilityError::DynamicCapabilityCall { .. })
        ));

        let shadowed = RhaiWorkspace::single_source("fn http_get(url) { url }");
        assert!(matches!(
            shadowed.validate_declared_capabilities(&[]),
            Err(super::RhaiWorkspaceCapabilityError::ReservedCapabilityHelper { .. })
        ));
    }
}
