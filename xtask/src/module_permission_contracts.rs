use super::*;

pub(crate) fn validate_module_permission_contract(slug: &str, module_root: &Path) -> Result<()> {
    let lib_path = module_root.join("src").join("lib.rs");
    if !lib_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&lib_path)
        .with_context(|| format!("Failed to read {}", lib_path.display()))?;
    if !content.contains("impl RusToKModule for") {
        return Ok(());
    }

    let Some(permission_body) = extract_runtime_method_body(&content, "permissions") else {
        return Ok(());
    };

    let permission_contract = load_core_permission_contract(
        &workspace_root()
            .join("crates")
            .join("libs")
            .join("rustok-core"),
    )?;
    let mut seen = HashSet::new();

    for permission in extract_permission_constants(permission_body) {
        let Some(canonical) = permission_contract.constants.get(&permission) else {
            anyhow::bail!(
                "Module '{slug}' declares unknown Permission::{} in {}",
                permission,
                lib_path.display()
            );
        };
        if !seen.insert(canonical.clone()) {
            anyhow::bail!(
                "Module '{slug}' declares duplicate permission '{}' in {}",
                canonical,
                lib_path.display()
            );
        }
    }

    for (resource, action) in extract_permission_constructors(permission_body) {
        if !permission_contract.resources.contains(&resource) {
            anyhow::bail!(
                "Module '{slug}' declares unknown Resource::{} in Permission::new(...) at {}",
                resource,
                lib_path.display()
            );
        }
        if !permission_contract.actions.contains(&action) {
            anyhow::bail!(
                "Module '{slug}' declares unknown Action::{} in Permission::new(...) at {}",
                action,
                lib_path.display()
            );
        }

        let canonical = format!(
            "{}:{}",
            type_name_to_permission_segment(&resource),
            type_name_to_permission_segment(&action)
        );
        if !seen.insert(canonical.clone()) {
            anyhow::bail!(
                "Module '{slug}' declares duplicate permission '{}' in {}",
                canonical,
                lib_path.display()
            );
        }
    }

    for expected in expected_minimum_module_permissions(slug) {
        if !seen.contains(*expected) {
            anyhow::bail!(
                "Module '{slug}' must declare minimum runtime permission '{}' in {}",
                expected,
                lib_path.display()
            );
        }
    }

    Ok(())
}

fn extract_permission_constants(content: &str) -> Vec<String> {
    regex::Regex::new(r"Permission::([A-Z0-9_]+)")
        .expect("permission constant regex should compile")
        .captures_iter(content)
        .map(|capture| capture[1].to_string())
        .collect()
}

fn extract_permission_constructors(content: &str) -> Vec<(String, String)> {
    regex::Regex::new(r"Permission::new\(Resource::([A-Za-z0-9_]+),\s*Action::([A-Za-z0-9_]+)\)")
        .expect("permission ctor regex should compile")
        .captures_iter(content)
        .map(|capture| (capture[1].to_string(), capture[2].to_string()))
        .collect()
}
