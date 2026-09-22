use super::*;

pub(crate) struct RuntimeModuleSource {
    pub(crate) path: PathBuf,
    pub(crate) content: String,
}

pub(crate) fn load_runtime_module_source(
    module_root: &Path,
) -> Result<Option<RuntimeModuleSource>> {
    let src_root = module_root.join("src");
    let mut matches = Vec::new();

    for file_name in ["module.rs", "lib.rs"] {
        let path = src_root.join(file_name);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        if extract_runtime_module_entry_type(&content).is_some() {
            matches.push(RuntimeModuleSource { path, content });
        }
    }

    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.pop()),
        _ => anyhow::bail!(
            "Module runtime contract is ambiguous: both {} and {} implement RusToKModule",
            src_root.join("module.rs").display(),
            src_root.join("lib.rs").display()
        ),
    }
}

pub(crate) fn extract_runtime_module_dependencies(
    module_root: &Path,
) -> Result<Option<HashSet<String>>> {
    let Some(source) = load_runtime_module_source(module_root)? else {
        return Ok(None);
    };
    let content = source.content;

    let marker = "fn dependencies(&self)";
    let Some(marker_index) = content.find(marker) else {
        return Ok(Some(HashSet::new()));
    };

    let tail = &content[marker_index..];
    let Some(body_start_offset) = tail.find('{') else {
        return Ok(Some(HashSet::new()));
    };
    let body = &tail[body_start_offset..];
    let Some(array_start_offset) = body.find("&[") else {
        return Ok(Some(HashSet::new()));
    };
    let array_tail = &body[(array_start_offset + 2)..];
    let Some(array_end_offset) = array_tail.find(']') else {
        anyhow::bail!(
            "Failed to parse RusToKModule::dependencies() in {}",
            source.path.display()
        );
    };
    let array_body = &array_tail[..array_end_offset];

    Ok(Some(
        extract_string_literals(array_body)
            .into_iter()
            .collect::<HashSet<_>>(),
    ))
}

pub(crate) fn infer_runtime_module_entry_type(module_root: &Path) -> Result<Option<String>> {
    Ok(load_runtime_module_source(module_root)?
        .and_then(|source| extract_runtime_module_entry_type(&source.content)))
}

pub(crate) fn extract_runtime_module_entry_type(content: &str) -> Option<String> {
    let marker = "impl RusToKModule for ";
    let start = content.find(marker)? + marker.len();
    let ident = content[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    if ident.is_empty() {
        return None;
    }

    Some(ident)
}

pub(crate) fn extract_runtime_module_kind(content: &str) -> Option<&'static str> {
    let body = extract_runtime_method_body(content, "kind")?;
    if body.contains("ModuleKind::Core") {
        return Some("Core");
    }
    if body.contains("ModuleKind::Optional") {
        return Some("Optional");
    }
    None
}

pub(crate) fn extract_runtime_method_body<'a>(
    content: &'a str,
    method_name: &str,
) -> Option<&'a str> {
    let marker = format!("fn {method_name}(&self)");
    let marker_index = content.find(&marker)?;
    let tail = &content[marker_index..];
    let body_start_offset = tail.find('{')?;
    let body = &tail[(body_start_offset + 1)..];
    let mut depth = 1usize;

    for (index, ch) in body.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&body[..index]);
                }
            }
            _ => {}
        }
    }

    None
}

pub(crate) fn extract_string_literals(input: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape = false;

    for ch in input.chars() {
        if !in_string {
            if ch == '"' {
                in_string = true;
                current.clear();
            }
            continue;
        }

        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        match ch {
            '\\' => escape = true,
            '"' => {
                values.push(current.clone());
                current.clear();
                in_string = false;
            }
            _ => current.push(ch),
        }
    }

    values
}

pub(crate) fn extract_runtime_string_method(content: &str, method_name: &str) -> Option<String> {
    let marker = format!("fn {method_name}(&self)");
    let marker_index = content.find(&marker)?;
    let tail = &content[marker_index..];
    let body_start_offset = tail.find('{')?;
    let body = &tail[body_start_offset..];
    let first_quote_offset = body.find('"')?;
    let quoted = &body[(first_quote_offset + 1)..];
    let end_quote_offset = quoted.find('"')?;
    Some(quoted[..end_quote_offset].to_string())
}
