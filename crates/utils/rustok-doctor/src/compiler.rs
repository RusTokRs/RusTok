use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{Confidence, Evidence, Finding, Severity, discovery::Target};

pub(crate) struct CargoDiagnostics {
    pub build_succeeded: bool,
    by_file: BTreeMap<PathBuf, Vec<Observation>>,
}

#[derive(Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
enum CargoMessage {
    CompilerMessage {
        message: Diagnostic,
    },
    BuildFinished {
        success: bool,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct Diagnostic {
    code: Option<Code>,
    level: String,
    spans: Vec<Span>,
}

#[derive(Deserialize)]
struct Code {
    code: String,
}

#[derive(Deserialize)]
struct Span {
    file_name: String,
    line_start: usize,
    line_end: usize,
    column_start: usize,
    is_primary: bool,
    text: Vec<SpanLine>,
}

#[derive(Deserialize)]
struct SpanLine {
    text: String,
}

struct Observation {
    lint: Lint,
    span: Span,
    code: String,
}

#[derive(Clone, Copy)]
struct Lint {
    id: &'static str,
    name: &'static str,
    severity: Severity,
    confidence: Confidence,
    message: &'static str,
    fix: &'static str,
}

fn lint(code: &str) -> Option<Lint> {
    Some(match code {
        "clippy::let_underscore_future" => Lint {
            id: "P-RUST-LET-UNDERSCORE-FUTURE",
            name: "Future discarded through a wildcard",
            severity: Severity::High,
            confidence: Confidence::High,
            message: "Clippy recognized a future immediately discarded through a wildcard. The discard does not poll the future.",
            fix: "Await the future or hand it to an owned task. Explicitly drop it when abandoning the work is intentional.",
        },
        "let_underscore_lock" => Lint {
            id: "P-RUST-LET-UNDERSCORE-LOCK",
            name: "Synchronization guard discarded through a wildcard",
            severity: Severity::High,
            confidence: Confidence::High,
            message: "Rust identified a synchronization guard immediately released by a wildcard binding.",
            fix: "Use a named binding whose lifetime covers the critical section. Explicitly drop the guard for intentional immediate release.",
        },
        "clippy::unused_io_amount" => Lint {
            id: "P-RUST-UNUSED-IO-AMOUNT",
            name: "Partial I/O count discarded",
            severity: Severity::High,
            confidence: Confidence::High,
            message: "Clippy identified an ignored byte count from an I/O operation that permits partial transfer.",
            fix: "Account for the returned byte count or use an operation that implements the required complete-transfer semantics.",
        },
        "static_mut_refs" => Lint {
            id: "P-RUST-STATIC-MUT-REFERENCE",
            name: "Reference to a mutable static",
            severity: Severity::Critical,
            confidence: Confidence::High,
            message: "Rust identified a reference to a mutable static. Its aliasing guarantees must hold for the reference's lifetime.",
            fix: "Use an immutable static with a locally enforced ownership or synchronization invariant.",
        },
        "clippy::await_holding_refcell_ref" => Lint {
            id: "P-RUST-AWAIT-HOLDING-REFCELL",
            name: "Dynamic borrow across an await",
            severity: Severity::High,
            confidence: Confidence::Heuristic,
            message: "Clippy flagged a RefCell borrow that may remain active across suspension and conflict with reentrant access.",
            fix: "End the borrow in an inner lexical scope before suspension. Check its actual lifetime when an explicit drop is present.",
        },
        "clippy::arc_with_non_send_sync" => Lint {
            id: "P-RUST-ARC-NONSEND-SYNC",
            name: "Arc with a non-Send or non-Sync inner type",
            severity: Severity::Medium,
            confidence: Confidence::Heuristic,
            message: "Clippy identified an Arc with an inner type that lacks Send or Sync. Check the owner's sharing requirements.",
            fix: "Use Rc for local ownership when the API permits it, or enforce the required Send and Sync guarantees on shared state.",
        },
        _ => return None,
    })
}

impl CargoDiagnostics {
    pub(crate) fn read(root: &Path, path: &Path, targets: &[Target]) -> Result<Self, String> {
        let input = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut selected = BTreeMap::new();
        for target in targets {
            let path = root.join(&target.path);
            let canonical =
                fs::canonicalize(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            selected.insert(path, &target.path);
            selected.entry(canonical).or_insert(&target.path);
        }
        let mut by_file: BTreeMap<PathBuf, Vec<Observation>> = BTreeMap::new();
        let mut build_succeeded = None;

        for (index, line) in BufReader::new(input).lines().enumerate() {
            let invalid = |error: &dyn std::fmt::Display| {
                format!("{}:{}: {error}", path.display(), index + 1)
            };
            let line = line.map_err(|error| invalid(&error))?;
            let line = if index == 0 {
                line.trim_start_matches('\u{feff}')
            } else {
                &line
            };
            if line.trim().is_empty() {
                continue;
            }
            if build_succeeded.is_some() {
                return Err(invalid(&"unexpected record after Cargo build-finished"));
            }
            match serde_json::from_str(line).map_err(|error| invalid(&error))? {
                CargoMessage::BuildFinished { success } => build_succeeded = Some(success),
                CargoMessage::CompilerMessage { message } => {
                    let Some(Code { code }) = message.code else {
                        continue;
                    };
                    if !matches!(message.level.as_str(), "error" | "warning") {
                        continue;
                    }
                    let Some(lint) = lint(&code) else {
                        continue;
                    };
                    for span in message.spans.into_iter().filter(|span| span.is_primary) {
                        if let Some(target) = selected.get(&root.join(&span.file_name)) {
                            by_file
                                .entry((*target).clone())
                                .or_default()
                                .push(Observation {
                                    lint,
                                    span,
                                    code: code.clone(),
                                });
                        }
                    }
                }
                CargoMessage::Other => {}
            }
        }

        Ok(Self {
            build_succeeded: build_succeeded.ok_or_else(|| {
                format!("{}: missing Cargo build-finished record", path.display())
            })?,
            by_file,
        })
    }

    pub(crate) fn findings(&self, path: &Path, source: &str) -> Result<Vec<Finding>, String> {
        let Some(observations) = self.by_file.get(path) else {
            return Ok(Vec::new());
        };
        let lines: Vec<_> = source.trim_start_matches('\u{feff}').lines().collect();
        let file = path.to_string_lossy().replace('\\', "/");
        let mut findings = BTreeMap::new();
        for Observation { lint, span, code } in observations {
            let first = span.line_start.saturating_sub(1);
            let matching_lines = lines.get(first..span.line_end).filter(|current| {
                span.line_start != 0
                    && span.column_start != 0
                    && !current.is_empty()
                    && current
                        .iter()
                        .copied()
                        .eq(span.text.iter().map(|line| line.text.as_str()))
            });
            let Some([line, ..]) = matching_lines
                .filter(|current| span.column_start <= current[0].chars().count() + 1)
            else {
                return Err(format!(
                    "{file}:{}: {code} diagnostic span does not match selected source; regenerate the Cargo diagnostics from this working tree",
                    span.line_start
                ));
            };
            let evidence = if code.starts_with("clippy::") {
                Evidence::Clippy {
                    code: code.clone(),
                    column: span.column_start,
                }
            } else {
                Evidence::Rustc {
                    code: code.clone(),
                    column: span.column_start,
                }
            };
            findings
                .entry((span.line_start, span.column_start, lint.id))
                .or_insert_with(|| Finding {
                    pattern_id: lint.id,
                    rule_name: lint.name,
                    severity: lint.severity,
                    confidence: lint.confidence,
                    file: file.clone(),
                    line: span.line_start,
                    content: line.trim().to_owned(),
                    message: lint.message.to_owned(),
                    suggested_fix: Some(lint.fix.to_owned()),
                    evidence,
                });
        }
        Ok(findings.into_values().collect())
    }
}
