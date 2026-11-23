use super::{safe_relative, DeclaredFiles};
use anyhow::{bail, ensure, Context, Result};
use proc_macro2::{TokenStream, TokenTree};
use std::collections::{BTreeMap, BTreeSet};
use syn::{
    ext::IdentExt, punctuated::Punctuated, visit::Visit, Attribute, Expr, ItemMod, Lit, LitStr,
    Meta, Token,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourceKind {
    Module,
    ExplicitModule,
    IncludedRust,
    Asset,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Source {
    path: String,
    kind: SourceKind,
}

pub(super) fn check(
    declaration: &DeclaredFiles,
    tree: &BTreeSet<&str>,
    blobs: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeSet<String>> {
    let mut pending: Vec<_> = declaration
        .entrypoints
        .iter()
        .map(|path| Source {
            path: path.clone(),
            kind: SourceKind::Module,
        })
        .collect();
    let mut visited = BTreeSet::new();
    while let Some(source) = pending.pop() {
        if visited.contains(&source) {
            continue;
        }
        if source.kind != SourceKind::Asset {
            let bytes = blobs
                .get(&source.path)
                .context("declared Rust source not captured")?;
            let text = std::str::from_utf8(bytes)
                .with_context(|| format!("Rust source {} is not UTF-8", source.path))?;
            let linked =
                scan(&source, text, tree).with_context(|| format!("in {}", source.path))?;
            for target in linked {
                ensure!(
                    declaration.files.contains(&target.path),
                    "{} references undeclared source {}",
                    source.path,
                    target.path
                );
                pending.push(target);
            }
        }
        visited.insert(source);
    }
    Ok(visited.into_iter().map(|input| input.path).collect())
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(directory, _)| directory)
}

fn join(directory: &str, name: &str) -> String {
    if directory.is_empty() {
        name.to_owned()
    } else {
        format!("{directory}/{name}")
    }
}

fn relative(directory: &str, value: &str) -> Result<String> {
    ensure!(
        !value.is_empty() && !value.starts_with('/') && !value.contains(['\\', ':']),
        "unsupported source path {value:?}"
    );
    // Parent components are allowed only before named components. Resolving
    // x/../ on paper can hide symlink traversal or a missing directory x.
    let mut parts: Vec<_> = directory
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    let mut named_component = false;
    for part in value.split('/') {
        match part {
            "." => (),
            ".." => ensure!(
                !named_component && parts.pop().is_some(),
                "unsupported parent source path {value:?}"
            ),
            "" => bail!("empty component in source path {value:?}"),
            _ => {
                parts.push(part);
                named_component = true;
            }
        }
    }
    let path = parts.join("/");
    if !path.is_empty() {
        safe_relative(&path)?;
    }
    Ok(path)
}

struct Directories {
    explicit: String,
    default: String,
}

struct Scanner<'a> {
    tree: &'a BTreeSet<&'a str>,
    file_directory: &'a str,
    module: Option<Directories>,
    links: Vec<Source>,
    error: Option<anyhow::Error>,
}

fn scan(source: &Source, text: &str, tree: &BTreeSet<&str>) -> Result<Vec<Source>> {
    let directory = parent(&source.path);
    let name = source
        .path
        .rsplit('/')
        .next()
        .context("missing Rust filename")?;
    let module = match source.kind {
        SourceKind::IncludedRust | SourceKind::Asset => None,
        _ => {
            let default = if source.kind == SourceKind::ExplicitModule || name == "mod.rs" {
                directory.to_owned()
            } else {
                join(
                    directory,
                    name.strip_suffix(".rs")
                        .context("expected Rust module filename")?,
                )
            };
            Some(Directories {
                explicit: directory.to_owned(),
                default,
            })
        }
    };
    let mut scanner = Scanner {
        tree,
        file_directory: directory,
        module,
        links: Vec::new(),
        error: None,
    };
    match syn::parse_file(text) {
        Ok(file) => scanner.visit_file(&file),
        Err(_) if source.kind == SourceKind::IncludedRust => scanner.visit_expr(
            &syn::parse_str(text)
                .context("cannot parse included Rust as items or an expression")?,
        ),
        Err(error) => return Err(error).context("cannot parse Rust source"),
    }
    match scanner.error {
        Some(error) => Err(error),
        None => Ok(scanner.links),
    }
}

fn path_attribute(attrs: &[Attribute]) -> Result<Option<String>> {
    let mut explicit = None;
    for attribute in attrs {
        let Some(name) = attribute.path().get_ident().map(IdentExt::unraw) else {
            continue;
        };
        if name == "cfg_attr" {
            if let Meta::List(list) = &attribute.meta {
                ensure!(
                    !mentions_path(list.tokens.clone()),
                    "conditional #[path] is unsupported"
                );
            }
        }
        if name != "path" {
            continue;
        }
        ensure!(explicit.is_none(), "duplicate #[path] attributes");
        let Meta::NameValue(value) = &attribute.meta else {
            bail!("expected a literal #[path]")
        };
        let Expr::Lit(value) = &value.value else {
            bail!("expected a literal #[path]")
        };
        let Lit::Str(value) = &value.lit else {
            bail!("expected a string #[path]")
        };
        explicit = Some(value.value());
    }
    Ok(explicit)
}

fn mentions_path(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Ident(ident) => ident.unraw() == "path",
        TokenTree::Group(group) => mentions_path(group.stream()),
        _ => false,
    })
}

impl Scanner<'_> {
    fn module(&mut self, item: &ItemMod) -> Result<()> {
        let directories = self
            .module
            .as_ref()
            .context("module declarations in include! fragments are unsupported")?;
        let explicit = path_attribute(&item.attrs)?
            .map(|path| relative(&directories.explicit, &path))
            .transpose()?;
        let name = item.ident.unraw().to_string();
        if item.content.is_some() {
            let directory = explicit.unwrap_or_else(|| join(&directories.default, &name));
            let previous = self.module.replace(Directories {
                explicit: directory.clone(),
                default: directory,
            });
            syn::visit::visit_item_mod(self, item);
            self.module = previous;
            return Ok(());
        }
        if let Some(path) = explicit {
            safe_relative(&path)?;
            self.links.push(Source {
                path,
                kind: SourceKind::ExplicitModule,
            });
        } else {
            let flat = join(&directories.default, &format!("{name}.rs"));
            let nested = join(&directories.default, &format!("{name}/mod.rs"));
            let path = match (
                self.tree.contains(flat.as_str()),
                self.tree.contains(nested.as_str()),
            ) {
                (true, false) => flat,
                (false, true) => nested,
                (true, true) => bail!("ambiguous module source {flat} and {nested}"),
                (false, false) => bail!("module source absent from Git tree: {flat} or {nested}"),
            };
            self.links.push(Source {
                path,
                kind: SourceKind::Module,
            });
        }
        for attr in &item.attrs {
            self.visit_attribute(attr);
        }
        Ok(())
    }

    fn include(&mut self, kind: SourceKind, tokens: TokenStream) -> Result<()> {
        let args =
            syn::parse::Parser::parse2(Punctuated::<LitStr, Token![,]>::parse_terminated, tokens)
                .context(
                "include requires a literal relative path; generated inputs are unsupported",
            )?;
        ensure!(args.len() == 1, "include requires exactly one literal path");
        let path = relative(self.file_directory, &args[0].value())?;
        safe_relative(&path)?;
        self.links.push(Source { path, kind });
        Ok(())
    }

    fn macro_tokens(&mut self, tokens: TokenStream, definition: bool) -> Result<()> {
        let mut tokens = tokens.into_iter().peekable();
        while let Some(token) = tokens.next() {
            match token {
                TokenTree::Group(group) => self.macro_tokens(group.stream(), definition)?,
                TokenTree::Ident(name) => {
                    let Some(kind) = include_kind(&name.unraw().to_string()) else {
                        continue;
                    };
                    if matches!(tokens.peek(), Some(TokenTree::Punct(punctuation)) if punctuation.as_char() == '!')
                    {
                        ensure!(
                            !definition,
                            "include paths inside macro definitions require expansion evidence"
                        );
                        tokens.next();
                        let Some(TokenTree::Group(args)) = tokens.next() else {
                            bail!("unsupported include inside macro tokens")
                        };
                        self.include(kind, args.stream())?;
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
}

fn include_kind(name: &str) -> Option<SourceKind> {
    match name {
        "include" => Some(SourceKind::IncludedRust),
        "include_str" | "include_bytes" => Some(SourceKind::Asset),
        _ => None,
    }
}

impl<'ast> Visit<'ast> for Scanner<'_> {
    fn visit_item_mod(&mut self, item: &'ast ItemMod) {
        if self.error.is_none() {
            if let Err(error) = self.module(item) {
                self.error = Some(error);
            }
        }
    }

    fn visit_macro(&mut self, expression: &'ast syn::Macro) {
        if self.error.is_some() {
            return;
        }
        let name = expression
            .path
            .segments
            .last()
            .map(|segment| segment.ident.unraw().to_string());
        let result = match name.as_deref().and_then(include_kind) {
            Some(kind) => self.include(kind, expression.tokens.clone()),
            None => self.macro_tokens(
                expression.tokens.clone(),
                name.as_deref() == Some("macro_rules"),
            ),
        };
        self.error = result.err();
    }

    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        if let Meta::List(list) = &attribute.meta {
            if self.error.is_none() {
                self.error = self.macro_tokens(list.tokens.clone(), false).err();
            }
        } else {
            syn::visit::visit_attribute(self, attribute);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(entries: &[(&str, &str)]) -> (DeclaredFiles, BTreeMap<String, Vec<u8>>) {
        let blobs: BTreeMap<_, _> = entries
            .iter()
            .map(|(path, body)| (path.to_string(), body.as_bytes().to_owned()))
            .collect();
        let declaration = DeclaredFiles {
            files: blobs.keys().cloned().collect(),
            entrypoints: BTreeSet::from(["src/step.rs".into()]),
        };
        (declaration, blobs)
    }

    #[test]
    fn follows_nested_modules_path_attributes_and_assets() -> Result<()> {
        let (declaration, blobs) = input(&[
            (
                "src/step.rs",
                r#"
                mod schema;
                mod inline { #[path = "manual.rs"] mod manual; }
                #[path = "shared/alternate.rs"] mod alternate;
                const ASSET: &[u8] = std::include_bytes!("../assets/logo.bin",);
                const SQL: &str = include!("generated/expression.in");
            "#,
            ),
            ("src/step/schema/mod.rs", "mod tables;"),
            (
                "src/step/schema/tables.rs",
                r#"fn statement() { format!("{}", include_str!("table.sql")); }"#,
            ),
            ("src/step/schema/table.sql", "SELECT 1;"),
            ("src/step/inline/manual.rs", ""),
            ("src/shared/alternate.rs", "mod helper;"),
            ("src/shared/helper.rs", ""),
            ("assets/logo.bin", "asset"),
            (
                "src/generated/expression.in",
                r#"include_str!("../../assets/statement.sql")"#,
            ),
            ("assets/statement.sql", "SELECT 2;"),
        ]);
        let tree = blobs.keys().map(String::as_str).collect();
        assert_eq!(check(&declaration, &tree, &blobs)?, declaration.files);
        Ok(())
    }

    #[test]
    fn rejects_omitted_sources_and_ambiguous_modules() {
        let (mut declaration, blobs) = input(&[
            ("src/step.rs", "mod schema;"),
            (
                "src/step/schema.rs",
                r#"const SQL: &str = include_str!("table.sql");"#,
            ),
            ("src/step/table.sql", "SELECT 1;"),
        ]);
        let mut tree: BTreeSet<_> = blobs.keys().map(String::as_str).collect();
        declaration.files.remove("src/step/table.sql");
        let error = check(&declaration, &tree, &blobs).unwrap_err();
        assert!(format!("{error:#}").contains("undeclared source src/step/table.sql"));
        tree.insert("src/step/schema/mod.rs");
        let error = check(&declaration, &tree, &blobs).unwrap_err();
        assert!(format!("{error:#}").contains("ambiguous module source"));
    }

    #[test]
    fn rejects_dynamic_paths_and_paths_outside_the_tree() {
        for (body, expected) in [
            (
                r#"const SQL: &str = include_str!(concat!(env!("OUT_DIR"), "/input.sql"));"#,
                "requires a literal",
            ),
            (
                r#"#[cfg_attr(feature = "alt", path = "other.rs")] mod child;"#,
                "conditional #[path]",
            ),
            (
                r#"const SQL: &str = include_str!("../../outside.sql");"#,
                "unsupported parent",
            ),
            (
                r#"const SQL: &str = include_str!("sub/../table.sql");"#,
                "unsupported parent",
            ),
            (
                r#"macro_rules! load { () => { include_str!("table.sql") }; }"#,
                "macro definitions require expansion evidence",
            ),
        ] {
            let (declaration, blobs) = input(&[("src/step.rs", body)]);
            let tree = blobs.keys().map(String::as_str).collect();
            let error = check(&declaration, &tree, &blobs).unwrap_err();
            assert!(format!("{error:#}").contains(expected), "{error:#}");
        }
    }
}
