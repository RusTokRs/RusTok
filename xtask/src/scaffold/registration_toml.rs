use super::options::ScaffoldOptions;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Component, Path, PathBuf};
use toml_edit::{Document, InlineTable, Item, TableLike, Value};

pub(super) const SERVER: &str = "apps/server/Cargo.toml";
pub(super) const DISTRIBUTION: &str = "crates/modules/rustok-distribution/Cargo.toml";
pub(super) const UDEPS_NORMAL: &[&str] =
    &["package", "metadata", "cargo-udeps", "ignore", "normal"];

pub(super) fn register(root: &Path, workspace_root: &Path, opts: &ScaffoldOptions) -> Result<()> {
    let name = opts.crate_name();
    let module_path = format!("crates/modules/{name}");
    edit(root, "modules.toml", |editor| {
        let manifest = rustok_manifest::parse_modules_manifest(editor.source)?;
        anyhow::ensure!(
            !manifest.modules.contains_key(&opts.slug),
            "Module '{}' is already registered in modules.toml",
            opts.slug
        );
        rustok_manifest::validate_modules_manifest(&manifest)?;
        for (slug, spec) in &manifest.modules {
            anyhow::ensure!(
                spec.crate_name != name
                    && !spec.path.as_deref().is_some_and(|path| {
                        same_path(&normalized_path(path), &normalized_path(&module_path))
                    }),
                "Module '{slug}' already claims crate '{name}' or path '{module_path}'"
            );
        }
        let entry = inline_table([
            ("crate", Value::from(name.as_str())),
            ("source", Value::from("path")),
            ("path", Value::from(module_path.as_str())),
        ]);
        let modules = editor.get(&["modules"]).and_then(Item::as_table);
        // Keep a native module above the existing extension group when they
        // are defined together. Identify the group by its runtime, not prose.
        let after_native = modules
            .filter(|table| !table.is_implicit() && !table.is_dotted())
            .filter(|_| {
                manifest
                    .modules
                    .values()
                    .any(|spec| spec.runtime == "extension")
            })
            .and_then(|table| {
                table
                    .get_values()
                    .into_iter()
                    .filter(|(keys, _)| {
                        manifest
                            .modules
                            .get(keys[0].get())
                            .is_some_and(|spec| spec.runtime != "extension")
                    })
                    .filter_map(|(_, value)| value.span().map(|span| span.end))
                    .max()
                    .or_else(|| table.span().map(|span| span.end))
            });
        editor.insert_at(&["modules"], &opts.slug, &entry, after_native)
    })?;

    edit(root, "Cargo.toml", |editor| {
        let members = editor.strings(&["workspace", "members"])?;
        let excludes = if editor.get(&["workspace", "exclude"]).is_some() {
            editor.strings(&["workspace", "exclude"])?
        } else {
            Vec::new()
        };
        let mut missing = Vec::new();
        for path in requested_members(opts) {
            if !workspace_has_member(workspace_root, &members, &excludes, &path)? {
                missing.push(path);
            }
        }
        if !missing.is_empty() {
            editor.append(&["workspace", "members"], &missing)?;
        }
        editor.insert(
            &["workspace", "dependencies"],
            &name,
            &inline_table([("path", Value::from(module_path.as_str()))]),
        )
    })?;

    if !opts.skip_server_wire {
        let feature = format!("mod-{}", opts.slug);
        for relative in [SERVER, DISTRIBUTION] {
            edit(root, relative, |editor| {
                let mut edges = vec![format!("dep:{name}")];
                if relative == SERVER {
                    edges.push(format!("rustok-distribution/{feature}"));
                    if editor.get(UDEPS_NORMAL).is_some() {
                        let ignore = editor.strings(UDEPS_NORMAL)?;
                        let count = ignore.iter().filter(|dep| **dep == name).count();
                        anyhow::ensure!(count <= 1, "Duplicate cargo-udeps entry '{name}'");
                        if count == 0 {
                            editor.append(UDEPS_NORMAL, std::slice::from_ref(&name))?;
                        }
                    }
                }
                let edges = edges.into_iter().collect::<toml_edit::Array>();
                editor.insert(&["features"], &feature, &Value::Array(edges))?;
                editor.insert(
                    &["dependencies"],
                    &name,
                    &inline_table([
                        ("workspace", Value::from(true)),
                        ("optional", Value::from(true)),
                    ]),
                )
            })?;
        }
    }
    Ok(())
}

fn inline_table<const N: usize>(entries: [(&str, Value); N]) -> Value {
    let mut table: InlineTable = entries.into_iter().collect();
    table.fmt();
    Value::InlineTable(table)
}

fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
    } else {
        a == b
    }
}

pub(super) fn requested_members(opts: &ScaffoldOptions) -> impl Iterator<Item = String> {
    let root = format!("crates/modules/{}", opts.crate_name());
    let admin = opts.ui.has_admin().then(|| format!("{root}/admin"));
    let storefront = opts
        .ui
        .has_storefront()
        .then(|| format!("{root}/storefront"));
    std::iter::once(root).chain(admin).chain(storefront)
}

pub(super) fn workspace_has_member(
    root: &Path,
    members: &[&str],
    excludes: &[&str],
    path: &str,
) -> Result<bool> {
    let target = normalized_path(path);
    let absolute = |path: &str| normalized_path(&root.join(path).to_string_lossy());
    let absolute_target = absolute(path);
    let manifest = absolute(&format!("{path}/Cargo.toml"));
    let matches = |pattern: &str, candidate: &Path| -> Result<bool> {
        let pattern = normalized_path(pattern);
        let pattern = glob::Pattern::new(&pattern.to_string_lossy()).with_context(|| {
            format!(
                "Invalid Cargo workspace path pattern '{}'",
                pattern.display()
            )
        })?;
        Ok(pattern.matches_path_with(
            candidate,
            glob::MatchOptions {
                require_literal_separator: true,
                ..glob::MatchOptions::new()
            },
        ))
    };
    // Cargo interprets exclusions as literal path prefixes. An existing
    // explicit member can override an exclusion; the scaffold must not add
    // such an override implicitly.
    if let Some(exclude) = excludes
        .iter()
        .find(|exclude| manifest.starts_with(absolute(exclude)))
    {
        anyhow::ensure!(
            members
                .iter()
                .any(|member| manifest.starts_with(absolute(member))),
            "Cargo workspace excludes requested module path '{path}' via '{exclude}'"
        );
    }
    let exact = members
        .iter()
        .filter(|member| same_path(&absolute(member), &absolute_target))
        .count();
    anyhow::ensure!(exact <= 1, "Duplicate Cargo workspace member '{path}'");
    let mut covered = exact != 0;
    for member in members {
        let candidate = if Path::new(member).is_absolute() {
            &absolute_target
        } else {
            &target
        };
        covered |= matches(member, candidate)?;
    }
    Ok(covered)
}

fn normalized_path(path: &str) -> PathBuf {
    let mut result = PathBuf::new();
    for part in Path::new(path).components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir if result.file_name().is_some_and(|name| name != "..") => {
                result.pop();
            }
            _ => result.push(part.as_os_str()),
        }
    }
    result
}

fn edit(
    root: &Path,
    relative: &str,
    update: impl FnOnce(&mut Editor<'_>) -> Result<()>,
) -> Result<()> {
    (|| -> Result<()> {
        let path = root.join(relative);
        let source = fs::read_to_string(&path)?;
        let mut editor = Editor {
            source: &source,
            document: Document::parse(source.as_str())?,
            insertions: Vec::new(),
        };
        update(&mut editor)?;
        let staged = editor.finish();
        fs::write(path, staged)?;
        Ok(())
    })()
    .with_context(|| format!("Cannot plan TOML registration in {relative}"))
}

/// Locate insertions with the TOML parser, then splice only new bytes. Rendering
/// the entire document would regroup interleaved dotted keys and normalize EOLs.
struct Editor<'a> {
    source: &'a str,
    document: Document<&'a str>,
    insertions: Vec<(usize, String)>,
}

impl Editor<'_> {
    fn get(&self, path: &[&str]) -> Option<&Item> {
        path.iter().try_fold(self.document.as_item(), |item, key| {
            item.as_table_like()?.get(key)
        })
    }

    fn strings(&self, path: &[&str]) -> Result<Vec<&str>> {
        let array = self
            .get(path)
            .and_then(Item::as_array)
            .with_context(|| format!("Expected array at {}", path.join(".")))?;
        array
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .with_context(|| format!("Expected string in {}", path.join(".")))
            })
            .collect()
    }

    fn insert(&mut self, path: &[&str], key: &str, value: &Value) -> Result<()> {
        self.insert_at(path, key, value, None)
    }

    fn insert_at(
        &mut self,
        path: &[&str],
        key: &str,
        value: &Value,
        preferred_end: Option<usize>,
    ) -> Result<()> {
        let table_at = |path: &[&str]| {
            self.get(path)
                .and_then(Item::as_table_like)
                .with_context(|| format!("Expected table at {}", path.join(".")))
        };
        anyhow::ensure!(
            !table_at(path)?.contains_key(key),
            "Registration key '{}.{key}' is already occupied",
            path.join(".")
        );
        // A dotted table is a logical node. Insert into its enclosing physical
        // table using the dotted suffix of the requested key.
        let mut depth = path.len();
        let mut table = table_at(path)?;
        while table.is_dotted() {
            depth -= 1;
            table = table_at(&path[..depth])?;
        }
        let qualified = path[depth..]
            .iter()
            .copied()
            .chain([key])
            .collect::<Vec<_>>()
            .join(".");
        let container = self
            .get(&path[..depth])
            .context("Missing TOML insertion table")?;
        let end = preferred_end.or_else(|| last_value_end(table));
        if let Some(inline) = container.as_inline_table() {
            let (offset, prefix) = match end {
                Some(end) => (end, ", "),
                None => (
                    inline.span().context("Missing inline table span")?.start + 1,
                    " ",
                ),
            };
            self.insertions
                .push((offset, format!("{prefix}{qualified} = {value}")));
        } else if let Some(anchor) = end.or_else(|| {
            container
                .as_table()
                .filter(|table| !table.is_implicit())?
                .span()
                .map(|span| span.end)
        }) {
            self.insert_line(anchor, &format!("{qualified} = {value}"));
        } else {
            // An implicit parent introduced by a child section has no header
            // of its own. TOML permits defining the parent after its children.
            let eol = self.line_ending(self.source.len());
            self.insert_line(
                self.source.len(),
                &format!("[{}]{eol}{qualified} = {value}", path[..depth].join(".")),
            );
        }
        Ok(())
    }

    fn append(&mut self, path: &[&str], values: &[String]) -> Result<()> {
        let array = self
            .get(path)
            .and_then(Item::as_array)
            .with_context(|| format!("Expected array at {}", path.join(".")))?;
        let span = array.span().context("Missing array span")?;
        let trailing_comma = array.trailing_comma();
        let formatted = values
            .iter()
            .map(|value| Value::from(value.as_str()).to_string())
            .collect::<Vec<_>>();
        let (offset, text) = if let Some(last) = array.iter().last().and_then(Value::span) {
            let close = span.end - 1;
            let closing_line = self.source[..close].rfind('\n').map_or(0, |i| i + 1);
            let value_line = self.source[..last.start].rfind('\n').map_or(0, |i| i + 1);
            let indent = &self.source[value_line..last.start];
            if closing_line > last.end
                && self.source[closing_line..close].trim().is_empty()
                && indent.bytes().all(|b| b == b' ' || b == b'\t')
            {
                let eol = self.line_ending(closing_line);
                if !trailing_comma {
                    // Keep any inline comment on its original array element.
                    self.insertions.push((last.end, ",".to_owned()));
                }
                (
                    closing_line,
                    formatted
                        .iter()
                        .enumerate()
                        .map(|(i, value)| {
                            let comma = if trailing_comma || i + 1 != formatted.len() {
                                ","
                            } else {
                                ""
                            };
                            format!("{indent}{value}{comma}{eol}")
                        })
                        .collect(),
                )
            } else {
                (last.end, format!(", {}", formatted.join(", ")))
            }
        } else {
            (span.start + 1, formatted.join(", "))
        };
        self.insertions.push((offset, text));
        Ok(())
    }

    fn line_ending(&self, offset: usize) -> &'static str {
        let newline = self.source[offset..]
            .find('\n')
            .map(|index| offset + index)
            .or_else(|| self.source[..offset].rfind('\n'));
        if newline.is_some_and(|index| index > 0 && self.source.as_bytes()[index - 1] == b'\r') {
            "\r\n"
        } else {
            "\n"
        }
    }

    fn insert_line(&mut self, anchor: usize, line: &str) {
        let eol = self.line_ending(anchor);
        let (offset, text) = match self.source[anchor..].find('\n') {
            Some(next) => (anchor + next + 1, format!("{line}{eol}")),
            None if self.source.ends_with('\n') => (self.source.len(), format!("{line}{eol}")),
            None => (self.source.len(), format!("{eol}{line}")),
        };
        self.insertions.push((offset, text));
    }

    fn finish(mut self) -> String {
        self.insertions.sort_by_key(|(offset, _)| *offset);
        let mut staged = self.source.to_owned();
        for (offset, text) in self.insertions.into_iter().rev() {
            staged.insert_str(offset, &text);
        }
        staged
    }
}

fn last_value_end(table: &dyn TableLike) -> Option<usize> {
    table
        .get_values()
        .into_iter()
        .filter_map(|(_, value)| value.span().map(|span| span.end))
        .max()
}
