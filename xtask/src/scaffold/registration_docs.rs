use super::options::{ScaffoldOptions, ScaffoldUi};
use crate::docs_markdown::{Document, Section, MODULE_COLUMNS, OPTIONAL_MODULES};
use anyhow::{Context, Result};
use pulldown_cmark::HeadingLevel::{H2, H3};
use std::{fs, ops::Range, path::Path};

pub(super) const REGISTRY: &str = "docs/modules/registry.md";
pub(super) const INDEX: &str = "docs/modules/_index.md";
pub(super) const PLANS: &str = "docs/modules/implementation-plans-registry.md";
pub(super) const UI_INDEX: &str = "docs/modules/UI_PACKAGES_INDEX.md";

const READINESS: Section = Section(H2, "FFA/FBA Readiness Board (Module-owned UI)");
const RELEASE: Section = Section(
    H2,
    "Module Release, Rollback and Transition Readiness Board",
);
const DOMAIN: Section = Section(H2, "Domain Modules");
const LIVE_PLANS: Section = Section(H2, "Live plans");

enum Target {
    Table(&'static [&'static str], Option<String>),
    List(String),
}

struct Entry {
    section: Section,
    target: Target,
    identity: String,
    line: String,
}

impl Entry {
    fn table(
        section: Section,
        columns: &'static [&'static str],
        identity: &str,
        line: String,
    ) -> Self {
        Self {
            section,
            target: Target::Table(columns, None),
            identity: identity.into(),
            line,
        }
    }

    fn locate<'a>(&self, document: &'a Document) -> Result<Location<'a>> {
        let (heading_end, section) = document.section(self.section)?;
        let (scope, anchor, separate, collisions): (_, _, _, Vec<_>) = match &self.target {
            Target::Table(columns, crate_name) => {
                let table = document.table(self.section, columns)?;
                let collisions = document
                    .tables
                    .iter()
                    .filter(|table| table.has_columns(columns))
                    .flat_map(|table| &table.rows)
                    .filter(|row| {
                        row.cells
                            .first()
                            .is_some_and(|cell| cell.text.trim() == self.identity)
                            || crate_name.as_ref().is_some_and(|name| {
                                row.cells
                                    .get(1)
                                    .is_some_and(|cell| cell.text.trim() == name)
                            })
                    })
                    .map(|row| &row.span)
                    .collect();
                (table.span.clone(), table.span.end, false, collisions)
            }
            Target::List(link) => {
                let last_item = document
                    .items
                    .iter()
                    .rfind(|item| section.contains(&item.span.start));
                let collisions =
                    document
                        .items
                        .iter()
                        .filter(|item| {
                            item.inline.links.contains(link)
                                || (section.contains(&item.span.start)
                                    && item.inline.text.split_whitespace().next().is_some_and(
                                        |name| name.trim_end_matches(':') == self.identity,
                                    ))
                        })
                        .map(|item| &item.span)
                        .collect();
                (
                    section,
                    last_item.map_or(heading_end, |item| item.span.end),
                    last_item.is_none(),
                    collisions,
                )
            }
        };
        Ok(Location {
            scope,
            anchor,
            separate,
            collisions,
        })
    }
}

struct Location<'a> {
    scope: Range<usize>,
    anchor: usize,
    separate: bool,
    collisions: Vec<&'a Range<usize>>,
}

pub(super) fn register(root: &Path, opts: &ScaffoldOptions) -> Result<()> {
    for (relative, entries) in projections(opts) {
        let path = root.join(relative);
        let mut source =
            fs::read_to_string(&path).with_context(|| format!("Cannot read {relative}"))?;
        let document = Document::parse(&source);
        let mut insertions = Vec::new();
        for entry in entries {
            let location = entry
                .locate(&document)
                .with_context(|| format!("Cannot plan {relative}"))?;
            anyhow::ensure!(
                location.collisions.is_empty(),
                "Markdown identity '{}' is already registered in {relative} ({})",
                entry.identity,
                entry.section.1
            );
            insertions.push(insert_line(
                &source,
                location.anchor,
                &entry.line,
                location.separate,
            ));
        }
        insertions.sort_by_key(|(offset, _)| *offset);
        for (offset, line) in insertions.into_iter().rev() {
            source.insert_str(offset, &line);
        }
        fs::write(path, source).with_context(|| format!("Cannot stage {relative}"))?;
    }
    Ok(())
}

pub(super) fn validate(root: &Path, opts: &ScaffoldOptions) -> Result<()> {
    for (relative, entries) in projections(opts) {
        let source = fs::read_to_string(root.join(relative))
            .with_context(|| format!("Cannot read {relative}"))?;
        let document = Document::parse(&source);
        for entry in entries {
            let location = entry
                .locate(&document)
                .with_context(|| format!("Cannot verify {relative}"))?;
            anyhow::ensure!(
                matches!(location.collisions.as_slice(), [span]
                if location.scope.contains(&span.start) && source[(*span).clone()].trim() == entry.line),
                "Incomplete Markdown registration of '{}' in {relative} ({})",
                entry.identity,
                entry.section.1
            );
        }
    }
    Ok(())
}

fn insert_line(source: &str, anchor: usize, line: &str, separate: bool) -> (usize, String) {
    // A parser block may include its terminal line ending and blank lines.
    let end = source[..anchor].trim_end().len();
    let newline = source[end..].find('\n').map(|offset| end + offset);
    let style = newline.or_else(|| source[..end].rfind('\n'));
    let eol = if style.is_some_and(|i| i > 0 && source.as_bytes()[i - 1] == b'\r') {
        "\r\n"
    } else {
        "\n"
    };
    let gap = if separate { eol } else { "" };
    match newline {
        Some(index) => (index + 1, format!("{gap}{line}{eol}{gap}")),
        None => (source.len(), format!("{eol}{gap}{line}")),
    }
}

fn projections(opts: &ScaffoldOptions) -> Vec<(&'static str, Vec<Entry>)> {
    let slug = opts.slug.as_str();
    let name = opts.crate_name();
    let base = format!("../../crates/modules/{name}");
    let plan = format!("{base}/docs/implementation-plan.md");
    let desc = escape_text(&opts.description);
    let (surfaces, shape) = draft_readiness(opts.ui);
    let registry = vec![
        Entry::table(
            READINESS,
            &[
                "Module slug",
                "UI surfaces",
                "FFA status",
                "FBA status",
                "Structural shape",
                "Source plan",
            ],
            slug,
            format!(
                "| `{slug}` | {surfaces} | `not_started` | `not_started` | {shape} | [Implementation plan]({plan}); {desc} |"
            ),
        ),
        Entry {
            target: Target::Table(MODULE_COLUMNS, Some(name.clone())),
            ..Entry::table(
                OPTIONAL_MODULES,
                MODULE_COLUMNS,
                slug,
                format!("| `{slug}` | `{name}` | none | {desc} |"),
            )
        },
        Entry::table(
            RELEASE,
            &[
                "Module slug",
                "Transition Class",
                "Data Boundary Owner",
                "Predecessor Standby",
                "Rollback Eligibility",
                "Recovery Invariants & Guards",
            ],
            slug,
            format!(
                "| `{slug}` | `Stateless` | `data boundary: none` | none | `DeniedMaintenanceOnly` | Draft identity; release and rollback execution unverified; [owner plan]({plan}) |"
            ),
        ),
    ];
    let mut index = vec![Entry::table(
        DOMAIN,
        &["Component", "Documentation", "Implementation Plan"],
        &name,
        format!("| `{name}` | [docs]({base}/docs/README.md) | [plan]({plan}) |"),
    )];
    let plans = vec![Entry::table(
        LIVE_PLANS,
        &["Module/Crate", "Local plan", "Status", "Nearest priority"],
        slug,
        format!(
            "| `{slug}` | [plan]({plan}) | `not_started` | Specify owner operations and invariants before persistence, transports or live UI. |"
        ),
    )];
    let mut ui = Vec::new();
    for (requested, surface, section) in [
        (
            opts.ui.has_admin(),
            "admin",
            Section(H3, "Optional/Admin Surfaces"),
        ),
        (
            opts.ui.has_storefront(),
            "storefront",
            Section(H3, "Optional/Storefront Surfaces"),
        ),
    ] {
        if requested {
            let link = format!("{base}/{surface}/README.md");
            let item = |label: &str| Entry {
                section,
                target: Target::List(link.clone()),
                identity: name.clone(),
                line: format!("- `{name}`{label} (draft, unmounted): [README]({link})"),
            };
            index.push(item(""));
            ui.push(item(&format!(" {surface} UI")));
        }
    }
    let mut files = vec![(REGISTRY, registry), (INDEX, index), (PLANS, plans)];
    if !ui.is_empty() {
        files.push((UI_INDEX, ui));
    }
    files
}

fn escape_text(text: &str) -> String {
    let mut escaped = String::new();
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '&' | '~' | '|'
        ) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

fn draft_readiness(ui: ScaffoldUi) -> (&'static str, &'static str) {
    match ui {
        ScaffoldUi::Dual => (
            "admin + storefront (draft, unmounted)",
            "`core_transport_ui`",
        ),
        ScaffoldUi::Admin => ("admin (draft, unmounted)", "`core_transport_ui`"),
        ScaffoldUi::Storefront => ("storefront (draft, unmounted)", "`core_transport_ui`"),
        ScaffoldUi::None => ("none", "`no_ui_boundary`"),
    }
}
