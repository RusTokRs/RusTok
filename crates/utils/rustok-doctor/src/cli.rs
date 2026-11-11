use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{Confidence, Finding, Severity, discovery, report, scan_source};

#[derive(Default)]
struct Options {
    hotspots: bool,
    top: Option<usize>,
    crate_names: Option<String>,
    path: Option<PathBuf>,
    all: bool,
    all_modules: bool,
    staged: bool,
    changed: bool,
    diff: Option<String>,
    summary_only: bool,
    strict: bool,
    severity: Option<String>,
    json: bool,
    suggest_fixes: bool,
    markdown: Option<PathBuf>,
    csv: Option<PathBuf>,
    patch: Option<PathBuf>,
    annotations: bool,
    fail_on: Option<String>,
    ignored: BTreeSet<String>,
    threads: Option<usize>,
}

fn usage() {
    println!(
        "Usage: cargo xtask doctor hotspots [--crate NAME | --path PATH | --all-modules | --all | --staged | --changed | --diff REF] [--top N] [--json]"
    );
    println!(
        "Usage: cargo xtask doctor hygiene [--crate NAME | --path PATH | --all-modules | --all | --staged | --changed | --diff REF]"
    );
    println!("  [--severity P0|P1|P2] [--strict | --fail-on P0|P1|P2] [--ignore-rule IDS]");
    println!("  [--json] [--summary-only] [--suggest-fixes] [--csv FILE]");
    println!("  [--markdown-report FILE] [--patch FILE] [--github-annotations] [--threads N]");
}

fn parse(args: &[String]) -> Result<Option<Options>, String> {
    let mut options = Options::default();
    let mut args = args.iter();
    while let Some(argument) = args.next() {
        let mut value = || {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{argument} requires a value"))
        };
        match argument.as_str() {
            "hygiene" => {}
            "hotspots" if !options.hotspots => options.hotspots = true,
            "--help" | "-h" => {
                usage();
                return Ok(None);
            }
            "--crate" => options.crate_names = Some(value()?),
            "--path" => options.path = Some(value()?.into()),
            "--all" => options.all = true,
            "--all-modules" => options.all_modules = true,
            "--staged" => options.staged = true,
            "--changed" => options.changed = true,
            "--diff" => options.diff = Some(value()?),
            "--summary-only" => options.summary_only = true,
            "--strict" => options.strict = true,
            "--severity" => options.severity = Some(value()?),
            "--json" => options.json = true,
            "--suggest-fixes" => options.suggest_fixes = true,
            "--markdown-report" => options.markdown = Some(value()?.into()),
            "--csv" => options.csv = Some(value()?.into()),
            "--patch" => options.patch = Some(value()?.into()),
            "--github-annotations" => options.annotations = true,
            "--fail-on" => options.fail_on = Some(value()?),
            "--ignore-rule" => {
                options.ignored = value()?
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_owned)
                    .collect()
            }
            "--threads" => {
                options.threads = Some(
                    value()?
                        .parse()
                        .map_err(|_| "threads must be a positive integer".to_owned())?,
                )
            }
            "--top" => {
                options.top = Some(
                    value()?
                        .parse()
                        .map_err(|_| "top must be a positive integer".to_owned())?,
                )
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    for threshold in [options.severity.as_deref(), options.fail_on.as_deref()]
        .into_iter()
        .flatten()
    {
        if !matches!(threshold, "P0" | "P1" | "P2") {
            return Err(format!("invalid severity: {threshold}"));
        }
    }
    if options.threads == Some(0) {
        return Err("threads must be a positive integer".to_owned());
    }
    if options.top == Some(0) || (options.top.is_some() && !options.hotspots) {
        return Err("--top requires hotspots and a positive integer".to_owned());
    }
    if options.hotspots
        && (options.strict
            || options.severity.is_some()
            || options.fail_on.is_some()
            || options.summary_only
            || options.suggest_fixes
            || options.csv.is_some()
            || options.markdown.is_some()
            || options.patch.is_some()
            || options.annotations
            || !options.ignored.is_empty()
            || options.threads.is_some())
    {
        return Err("hotspots accepts selection, --top and --json options".to_owned());
    }
    if options.json && options.annotations {
        return Err("--json and --github-annotations cannot be combined".to_owned());
    }
    Ok(Some(options))
}

fn root() -> Result<PathBuf, String> {
    let cwd = env::current_dir().map_err(|error| error.to_string())?;
    let bytes = match discovery::git(&cwd, &["rev-parse", "--show-toplevel"]) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(cwd),
    };
    let path = String::from_utf8(bytes).map_err(|error| error.to_string())?;
    Ok(PathBuf::from(path.trim()))
}

fn targets(root: &Path, options: &Options) -> Result<(Vec<discovery::Target>, String), String> {
    if options.staged {
        return Ok((discovery::staged(root)?, "git staged files".to_owned()));
    }
    if options.changed {
        return Ok((
            discovery::changed(root, None)?,
            "working tree modified files".to_owned(),
        ));
    }
    if let Some(reference) = &options.diff {
        return Ok((
            discovery::changed(root, Some(reference))?,
            format!("git diff against {reference}"),
        ));
    }
    let mut files = BTreeSet::new();
    let description;
    if let Some(names) = &options.crate_names {
        for path in discovery::crates(root, names)? {
            discovery::walk(root, &path, &mut files)?;
        }
        description = format!("crates: {names}");
    } else if let Some(path) = &options.path {
        let selected = root.join(path);
        if !selected.exists() {
            return Err(format!("Path does not exist: {}", selected.display()));
        }
        discovery::walk(root, &selected, &mut files)?;
        description = format!("path: {}", path.display());
    } else if options.all {
        for name in ["crates", "apps", "xtask"] {
            let path = root.join(name);
            if path.exists() {
                discovery::walk(root, &path, &mut files)?;
            }
        }
        description = "workspace (all crates & apps)".to_owned();
    } else if options.all_modules {
        discovery::walk(root, &root.join("crates/modules"), &mut files)?;
        description = "crates/modules (all)".to_owned();
    } else {
        let cwd = env::current_dir().map_err(|error| error.to_string())?;
        if cwd == root {
            discovery::walk(root, &root.join("crates/modules"), &mut files)?;
            description = "crates/modules (default)".to_owned();
        } else {
            discovery::walk(root, &cwd, &mut files)?;
            description = format!(
                "directory: {}",
                cwd.strip_prefix(root).unwrap_or(&cwd).display()
            );
        }
    }
    Ok((
        files
            .into_iter()
            .map(|path| discovery::Target { path, blob: None })
            .collect(),
        description,
    ))
}

fn scan(
    root: &Path,
    targets: &[discovery::Target],
    threads: usize,
    capture_sources: bool,
) -> Result<Vec<(Vec<Finding>, Option<String>)>, String> {
    let workers = if targets.len() > 20 {
        threads.min(targets.len()).max(1)
    } else {
        1
    };
    let chunk_size = targets.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = targets
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|target| {
                            let source = discovery::read(root, target)?;
                            let findings = scan_source(&target.path, &source);
                            let source =
                                (capture_sources && !findings.is_empty()).then_some(source);
                            Ok((findings, source))
                        })
                        .collect::<Result<Vec<_>, String>>()
                })
            })
            .collect();
        let mut output = Vec::new();
        for handle in handles {
            output.extend(
                handle
                    .join()
                    .map_err(|_| "scanner worker failed".to_owned())??,
            );
        }
        Ok(output)
    })
}

fn threshold(severity: Severity, limit: &str) -> bool {
    let maximum = match limit {
        "P0" => Severity::Critical,
        "P1" => Severity::High,
        _ => Severity::Medium,
    };
    severity <= maximum
}

fn output_identity(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        return fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()));
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid output path: {}", path.display()))?;
    let name = path
        .file_name()
        .ok_or_else(|| format!("invalid output path: {}", path.display()))?;
    Ok(fs::canonicalize(parent)
        .map_err(|error| format!("{}: {error}", parent.display()))?
        .join(name))
}

fn validate_outputs(
    root: &Path,
    targets: &[discovery::Target],
    options: &Options,
) -> Result<(), String> {
    if options.csv.is_none() && options.markdown.is_none() && options.patch.is_none() {
        return Ok(());
    }
    let mut sources = BTreeSet::new();
    for target in targets {
        let path = root.join(&target.path);
        if path.exists() || path.parent().is_some_and(Path::exists) {
            sources.insert(output_identity(&path)?);
        }
    }
    let mut outputs = BTreeSet::new();
    for path in [&options.csv, &options.markdown, &options.patch]
        .into_iter()
        .flatten()
    {
        let identity = output_identity(&root.join(path))?;
        if sources.contains(&identity) || !outputs.insert(identity) {
            return Err(format!(
                "output path conflicts with another output or scanned source: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn execute(options: Options) -> Result<i32, String> {
    let root = root()?;
    let (targets, description) = targets(&root, &options)?;
    if options.hotspots {
        return crate::hotspots::run(
            &root,
            &targets,
            &description,
            options.top.unwrap_or(20),
            options.json,
        );
    }
    validate_outputs(&root, &targets, &options)?;
    let count = targets.len();
    let threads = options
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |value| value.get()));
    let mut findings = Vec::new();
    let mut sources = BTreeMap::new();
    for (index, (items, content)) in scan(&root, &targets, threads, options.patch.is_some())?
        .into_iter()
        .enumerate()
    {
        if let Some(content) = content {
            sources.insert(targets[index].path.clone(), content);
        }
        findings.extend(items);
    }
    findings.retain(|finding| {
        !options.ignored.contains(finding.pattern_id)
            && options
                .severity
                .as_deref()
                .is_none_or(|limit| threshold(finding.severity, limit))
    });
    let fail_on = if options.strict {
        Some("P2")
    } else {
        options.fail_on.as_deref()
    };
    let exit = i32::from(fail_on.is_some_and(|limit| {
        findings.iter().any(|finding| {
            finding.confidence == Confidence::High && threshold(finding.severity, limit)
        })
    }));
    if let Some(path) = &options.csv {
        report::write(&root, path, &report::csv(&findings)?)?;
    }
    if let Some(path) = &options.markdown {
        report::write(
            &root,
            path,
            &report::markdown(&findings, count, &description),
        )?;
    }
    if let Some(path) = &options.patch {
        report::write(&root, path, &report::patch(&findings, &sources)?)?;
    }
    if options.json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "target": description, "scanned_files": count, "total_findings": findings.len(), "findings": findings
        })).map_err(|error| error.to_string())?);
        return Ok(exit);
    }
    report::print(
        &findings,
        count,
        &description,
        options.summary_only,
        options.suggest_fixes,
        options.all || options.all_modules,
    );
    if options.annotations {
        report::annotations(&findings);
    }
    Ok(exit)
}

/// Run the doctor CLI and return the diagnostic or operational exit code.
pub fn run(args: &[String]) -> i32 {
    match parse(args).and_then(|options| options.map_or(Ok(0), execute)) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("[ERROR] {error}");
            2
        }
    }
}
