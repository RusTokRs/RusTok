use super::*;

#[derive(Debug, Default)]
pub(crate) struct CorePermissionContract {
    pub(crate) constants: HashMap<String, String>,
    pub(crate) resources: HashSet<String>,
    pub(crate) actions: HashSet<String>,
}

pub(crate) fn load_core_permission_contract(core_root: &Path) -> Result<CorePermissionContract> {
    let mut permissions_path = core_root.join("src").join("permissions.rs");
    if !permissions_path.exists() {
        if let Some(crates_root) = core_root.parent() {
            let api_permissions_path = crates_root
                .join("rustok-api")
                .join("src")
                .join("permissions.rs");
            if api_permissions_path.exists() {
                permissions_path = api_permissions_path;
            }
        }
    }
    let content = fs::read_to_string(&permissions_path)
        .with_context(|| format!("Failed to read {}", permissions_path.display()))?;

    let mut contract = CorePermissionContract::default();
    let explicit_constant = regex::Regex::new(
        r"pub const ([A-Z0-9_]+): Self = Self::new\(Resource::([A-Za-z0-9_]+), Action::([A-Za-z0-9_]+)\);",
    )
    .expect("permission constant regex should compile");
    let macro_constant = regex::Regex::new(
        r"(?m)^\s*([A-Z0-9_]+)\s*=>\s*\(([A-Za-z0-9_]+),\s*([A-Za-z0-9_]+)\),\s*$",
    )
    .expect("permission macro constant regex should compile");

    for captures in explicit_constant
        .captures_iter(&content)
        .chain(macro_constant.captures_iter(&content))
    {
        contract.constants.insert(
            captures[1].to_string(),
            format!(
                "{}:{}",
                type_name_to_permission_segment(&captures[2]),
                type_name_to_permission_segment(&captures[3])
            ),
        );
    }

    contract.resources = extract_enum_variants(&content, "pub enum Resource");
    contract.actions = extract_enum_variants(&content, "pub enum Action");
    Ok(contract)
}

pub(crate) fn expected_minimum_module_permissions(slug: &str) -> &'static [&'static str] {
    match slug {
        "auth" => &["users:manage"],
        "tenant" => &["tenants:manage", "modules:manage"],
        "rbac" => &["settings:manage", "logs:read"],
        "blog" => &["blog_posts:manage", "blog_categories:manage"],
        "forum" => &["forum_topics:manage"],
        "customer" => &["customers:manage"],
        "product" => &["products:manage"],
        "profiles" => &["profiles:manage"],
        "region" => &["regions:manage"],
        "order" => &["orders:manage"],
        "payment" => &["payments:manage"],
        "fulfillment" => &["fulfillments:manage"],
        "media" => &["media:manage"],
        "pages" => &["pages:manage"],
        "taxonomy" => &["taxonomy:manage"],
        "workflow" => &["workflows:manage"],
        "alloy" => &["scripts:manage"],
        "flex" => &["flex_schemas:manage", "flex_entries:manage"],
        _ => &[],
    }
}

fn extract_enum_variants(content: &str, enum_marker: &str) -> HashSet<String> {
    let Some(marker_index) = content.find(enum_marker) else {
        return HashSet::new();
    };
    let tail = &content[marker_index..];
    let Some(body_start_offset) = tail.find('{') else {
        return HashSet::new();
    };
    let body = &tail[(body_start_offset + 1)..];
    let mut depth = 1usize;
    let mut end_index = None;
    for (index, ch) in body.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_index = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end_index) = end_index else {
        return HashSet::new();
    };
    let body = &body[..end_index];

    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .filter_map(|line| line.strip_suffix(','))
        .filter(|line| {
            line.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })
        .map(|line| line.to_string())
        .collect()
}

pub(crate) fn type_name_to_permission_segment(value: &str) -> String {
    let mut result = String::new();
    for (index, ch) in value.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}
