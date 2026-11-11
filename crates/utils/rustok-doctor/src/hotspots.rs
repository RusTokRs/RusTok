use std::path::Path;

use crate::discovery;

pub(crate) fn run(
    root: &Path,
    targets: &[discovery::Target],
    description: &str,
    top: usize,
    json: bool,
) -> Result<i32, String> {
    let mut files = Vec::new();
    let mut excluded_generated = 0usize;
    let mut excluded_vendor = 0usize;
    for target in targets {
        let vendor = target.path.components().any(|part| {
            let name = part.as_os_str().to_string_lossy().to_ascii_lowercase();
            name == "vendor"
                || name == "vendored"
                || name.starts_with("vendored-")
                || name.starts_with("vendored_")
        });
        if vendor {
            excluded_vendor += 1;
            continue;
        }
        let source = discovery::read(root, target)?;
        let generated = source.lines().take(20).any(|line| {
            let line = line.to_ascii_lowercase();
            let trimmed = line.trim_start();
            (trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*'))
                && (line.contains("@generated")
                    || line.contains("do not edit")
                    || line.contains("automatically generated"))
        });
        if generated {
            excluded_generated += 1;
            continue;
        }
        let test = target.path.components().any(|part| {
            matches!(
                part.as_os_str().to_str(),
                Some("tests" | "benches" | "fixtures")
            )
        }) || target
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name == "test.rs"
                    || name == "tests.rs"
                    || name.ends_with("_test.rs")
                    || name.ends_with("_tests.rs")
            });
        files.push((
            target.path.to_string_lossy().replace('\\', "/"),
            source.lines().count(),
            test,
        ));
    }
    files.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let total = files.len();
    files.truncate(top);
    if json {
        let files: Vec<_> = files
            .iter()
            .map(|(path, lines, test)| {
                serde_json::json!({
                    "file": path, "lines": lines, "kind": if *test { "test" } else { "source" }
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "target": description, "selected_files": targets.len(), "ranked_files": total,
            "excluded_generated": excluded_generated, "excluded_vendor": excluded_vendor, "files": files
        })).map_err(|error| error.to_string())?);
    } else {
        println!("Large Rust files — {description}");
        println!(
            "Ranked {total} files, excluded {excluded_generated} generated and {excluded_vendor} vendored files."
        );
        for (path, lines, test) in files {
            println!(
                "{lines:>7}  {}  {path}",
                if test { "test  " } else { "source" }
            );
        }
    }
    Ok(0)
}
