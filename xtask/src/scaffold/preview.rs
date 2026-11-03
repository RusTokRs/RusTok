use super::PlannedFile;
use anyhow::{Context, Result};
use diff::Result::{Both, Left, Right};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::ops::Range;
use std::path::Path;

const CONTEXT_LINES: usize = 3;
// The line diff allocates a quadratic table. Large files still get a full
// replacement hunk so the preview remains complete with bounded diff memory.
const MAX_DIFF_CELLS: usize = 1_000_000;

pub(super) fn write_preview(
    output: &mut impl Write,
    root: &Path,
    plan: &[PlannedFile],
) -> Result<()> {
    for file in plan {
        let path = file
            .path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        let original = match &file.original {
            Some(bytes) => {
                let original =
                    std::str::from_utf8(bytes).context("Scaffold original must be UTF-8")?;
                let digest = hex::encode(Sha256::digest(bytes));
                writeln!(output, "  [UPDATE] {path} (original SHA-256 {digest})")?;
                writeln!(output, "--- a/{path}")?;
                original
            }
            None => {
                writeln!(output, "  [CREATE] {path} (path must be absent)")?;
                writeln!(output, "--- /dev/null")?;
                ""
            }
        };
        writeln!(output, "+++ b/{path}")?;
        write_hunks(output, original, &file.content)?;
    }
    Ok(())
}

fn write_hunks(output: &mut impl Write, original: &str, planned: &str) -> std::io::Result<()> {
    let before: Vec<_> = original.split_inclusive('\n').collect();
    let after: Vec<_> = planned.split_inclusive('\n').collect();
    let prefix = before
        .iter()
        .zip(&after)
        .take_while(|(left, right)| left == right)
        .count();
    let suffix = before[prefix..]
        .iter()
        .rev()
        .zip(after[prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let before_end = before.len() - suffix;
    let before_change = &before[prefix..before_end];
    let after_change = &after[prefix..after.len() - suffix];
    let cells = (before_change.len() + 1).saturating_mul(after_change.len() + 1);
    let mut changes: Vec<_> = before[..prefix]
        .iter()
        .map(|line| Both(line, line))
        .collect();
    if cells <= MAX_DIFF_CELLS {
        changes.extend(diff::slice(before_change, after_change));
    } else {
        changes.extend(
            before_change
                .iter()
                .map(Left)
                .chain(after_change.iter().map(Right)),
        );
    }
    changes.extend(before[before_end..].iter().map(|line| Both(line, line)));

    let mut ranges: Vec<Range<usize>> = Vec::new();
    for (index, change) in changes.iter().enumerate() {
        if matches!(change, Both(..)) {
            continue;
        }
        let start = index.saturating_sub(CONTEXT_LINES);
        let end = (index + CONTEXT_LINES + 1).min(changes.len());
        match ranges.last_mut() {
            Some(range) if start <= range.end => range.end = end,
            _ => ranges.push(start..end),
        }
    }

    let mut old_position = 0;
    let mut new_position = 0;
    let mut previous_end = 0;
    for range in ranges {
        let skipped = range.start - previous_end;
        old_position += skipped;
        new_position += skipped;
        let hunk = &changes[range.clone()];
        let old_count = hunk.iter().filter(|line| !matches!(line, Right(_))).count();
        let new_count = hunk.iter().filter(|line| !matches!(line, Left(_))).count();
        writeln!(
            output,
            "@@ -{},{} +{},{} @@",
            old_position + usize::from(old_count != 0),
            old_count,
            new_position + usize::from(new_count != 0),
            new_count,
        )?;
        for line in hunk {
            let (prefix, text) = match line {
                Both(text, _) => (b' ', text),
                Left(text) => (b'-', text),
                Right(text) => (b'+', text),
            };
            output.write_all(&[prefix])?;
            output.write_all(text.as_bytes())?;
            if !text.ends_with('\n') {
                output.write_all(b"\n\\ No newline at end of file\n")?;
            }
        }
        old_position += old_count;
        new_position += new_count;
        previous_end = range.end;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn preview_shows_create_and_update_preconditions_with_exact_line_bytes() {
        let plan = [
            PlannedFile {
                path: PathBuf::from("workspace").join("src/lib.rs"),
                original: None,
                content: "pub struct Module;\n".into(),
            },
            PlannedFile {
                path: PathBuf::from("workspace").join("modules.toml"),
                original: Some(b"keep\r\nold".to_vec()),
                content: "keep\r\nnew\r\n".into(),
            },
        ];
        let mut output = Vec::new();
        write_preview(&mut output, Path::new("workspace"), &plan).unwrap();
        let digest = hex::encode(Sha256::digest(b"keep\r\nold"));
        let expected = format!(
            concat!(
                "  [CREATE] src/lib.rs (path must be absent)\n",
                "--- /dev/null\n+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+pub struct Module;\n",
                "  [UPDATE] modules.toml (original SHA-256 {})\n",
                "--- a/modules.toml\n+++ b/modules.toml\n@@ -1,2 +1,2 @@\n",
                " keep\r\n-old\n\\ No newline at end of file\n+new\r\n",
            ),
            digest
        );
        assert_eq!(output, expected.as_bytes());
    }

    #[test]
    fn separated_hunks_count_insertions_and_omit_distant_context() {
        let middle: String = (1..=9).map(|line| format!("line {line}\n")).collect();
        let before = format!("{middle}old\n");
        let after = format!("added\n{middle}new\n");
        let mut output = Vec::new();
        write_hunks(&mut output, &before, &after).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            concat!(
                "@@ -1,3 +1,4 @@\n+added\n line 1\n line 2\n line 3\n",
                "@@ -7,4 +8,4 @@\n line 7\n line 8\n line 9\n-old\n+new\n",
            )
        );
    }

    #[test]
    fn oversized_diffs_show_every_line_without_quadratic_allocation() {
        let before = "old\n".repeat(1_000);
        let after = "new\n".repeat(1_000);
        let mut output = Vec::new();
        write_hunks(&mut output, &before, &after).unwrap();
        let expected = format!(
            "@@ -1,1000 +1,1000 @@\n{}{}",
            "-old\n".repeat(1_000),
            "+new\n".repeat(1_000)
        );
        assert_eq!(output, expected.as_bytes());
    }

    #[test]
    fn a_small_edit_in_a_large_file_keeps_a_small_preview() {
        let common = "same\n".repeat(2_000);
        let mut output = Vec::new();
        write_hunks(
            &mut output,
            &format!("{common}old\n{common}"),
            &format!("{common}new\n{common}"),
        )
        .unwrap();
        assert_eq!(
            output,
            b"@@ -1998,7 +1998,7 @@\n same\n same\n same\n-old\n+new\n same\n same\n same\n"
        );
    }
}
