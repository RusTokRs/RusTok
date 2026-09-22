use super::*;

pub(crate) fn collect_module_source_files(module_root: &Path) -> Result<Vec<(PathBuf, String)>> {
    let src_root = module_root.join("src");
    if !src_root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_rust_source_files_recursive(&src_root, &mut files)?;
    Ok(files)
}

fn collect_rust_source_files_recursive(
    dir: &Path,
    files: &mut Vec<(PathBuf, String)>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry =
            entry.with_context(|| format!("Failed to read entry under {}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_files_recursive(&path, files)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        files.push((path, content));
    }

    Ok(())
}

pub(crate) fn provided_path_symbol(path: &str) -> Result<&str> {
    let symbol = path
        .trim()
        .rsplit("::")
        .next()
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("Failed to resolve symbol from declared path '{path}'"))?;
    Ok(symbol)
}

pub(crate) fn validate_declared_symbol_exists(
    slug: &str,
    field: &str,
    declared_path: &str,
    symbol: &str,
    source_files: &[(PathBuf, String)],
) -> Result<()> {
    let found = source_files.iter().any(|(_, content)| {
        content.contains(&format!("pub struct {symbol}"))
            || content.contains(&format!("struct {symbol}"))
            || content.contains(&format!("pub enum {symbol}"))
            || content.contains(&format!("enum {symbol}"))
            || content.contains(&format!("pub fn {symbol}("))
            || content.contains(&format!("fn {symbol}("))
            || content.contains(symbol)
    });

    if !found {
        anyhow::bail!(
            "Module '{slug}' declares {field}='{}', but symbol '{}' was not found under src/**/*.rs",
            declared_path,
            symbol
        );
    }

    Ok(())
}
