use crate::model::{BlogCategoryDetail, BlogCategoryDraft, BlogCategoryUpdate};
use rustok_api::WritePathIssue;
use rustok_ui_core::{ui_busy_key, ui_busy_key_with_id};
use rustok_ui_forms::FormValidator;

/// Which module-owned workspace the Blog admin root renders for a subpath.
///
/// The host serves every module child page through one route; the module owns
/// the subpath dispatch (same shape as the Forum admin root).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlogAdminSection {
    Posts,
    Categories,
}

pub fn blog_admin_section_for_subpath(subpath: Option<&str>) -> BlogAdminSection {
    if subpath.is_some_and(|sub| sub == "categories" || sub.starts_with("categories/")) {
        BlogAdminSection::Categories
    } else {
        BlogAdminSection::Posts
    }
}

/// Display row for the category management table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogCategoryAdminTableRowViewModel {
    pub id: String,
    pub name: String,
    pub slug: String,
    /// Empty string when the category has no description.
    pub description: String,
    pub position: i32,
    pub locale: String,
}

pub fn blog_category_admin_table_row_view(
    category: &BlogCategoryDetail,
) -> BlogCategoryAdminTableRowViewModel {
    BlogCategoryAdminTableRowViewModel {
        id: category.id.clone(),
        name: category.name.clone(),
        slug: category.slug.clone(),
        description: category.description.clone().unwrap_or_default(),
        position: category.position,
        locale: category.effective_locale.clone(),
    }
}

/// Client-side search over the loaded categories, matching the Next admin
/// filter: name, slug or description substring, case-insensitive.
pub fn blog_category_matches_query(
    category: &BlogCategoryAdminTableRowViewModel,
    query: &str,
) -> bool {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return true;
    }
    category.name.to_lowercase().contains(&needle)
        || category.slug.to_lowercase().contains(&needle)
        || category.description.to_lowercase().contains(&needle)
}

/// Match the Blog owner's Unicode name bounds instead of inventing a client-only minimum.
pub const MIN_BLOG_CATEGORY_NAME_LENGTH: usize = 1;

pub fn blog_category_name_is_valid(name: &str) -> bool {
    FormValidator::new()
        .required("name", name, "Name is required")
        .min_length(
            "name",
            name.trim(),
            MIN_BLOG_CATEGORY_NAME_LENGTH,
            "Name is required",
        )
        .max_length("name", name.trim(), 120, "Name is too long")
        .finish()
        .is_ok()
}

pub fn busy_key_for_category_save(category_id: Option<&str>) -> String {
    category_id
        .map(|id| ui_busy_key_with_id("category-save", id))
        .unwrap_or_else(|| ui_busy_key("category-create"))
}

pub fn busy_key_for_category_delete(category_id: &str) -> String {
    ui_busy_key_with_id("category-delete", category_id)
}

#[derive(Debug, Clone)]
pub enum BlogCategorySaveOperation {
    Create(BlogCategoryDraft),
    Update {
        category_id: String,
        update: BlogCategoryUpdate,
    },
}

#[derive(Debug, Clone)]
pub struct BlogCategorySaveCommand {
    pub operation: BlogCategorySaveOperation,
    pub busy_key: String,
}

pub struct BlogCategoryFormInput<'a> {
    pub editing_category_id: Option<&'a str>,
    pub loaded_category: Option<&'a BlogCategoryDetail>,
    pub locale: &'a str,
    pub name: &'a str,
    pub slug: &'a str,
    pub description: &'a str,
    pub parent_id: Option<&'a str>,
    pub position: &'a str,
}

fn optional_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Validates the form and builds the create or update command. Validation
/// matches the owner name bounds and requires a target-language identity.
/// The create-only position must parse as a non-negative
/// integer (the owner additionally clamps the position to the sibling count).
pub fn prepare_blog_category_save_command(
    input: BlogCategoryFormInput<'_>,
    name_too_short_message: String,
    invalid_position_message: String,
    slug_required_message: String,
) -> Result<BlogCategorySaveCommand, WritePathIssue> {
    let name = input.name.trim().to_string();
    if (input.editing_category_id.is_none() || !name.is_empty())
        && !blog_category_name_is_valid(&name)
    {
        return Err(WritePathIssue::new(name_too_short_message));
    }
    let slug = optional_trimmed(input.slug);
    let locale = input.locale.trim().to_string();

    let busy_key = busy_key_for_category_save(input.editing_category_id);
    let operation = match input.editing_category_id {
        Some(category_id) => {
            let loaded = input
                .loaded_category
                .filter(|category| category.id == category_id && category.locale == locale)
                .ok_or_else(|| WritePathIssue::new(name_too_short_message.clone()))?;
            let own_copy = loaded.locale == loaded.effective_locale;
            if own_copy && !loaded.slug.is_empty() && slug.is_none() {
                return Err(WritePathIssue::new(slug_required_message));
            }
            let baseline_name = if own_copy { loaded.name.as_str() } else { "" };
            let baseline_description = if own_copy {
                loaded.description.as_deref()
            } else {
                None
            };
            let description = super::text_patch(Some(input.description), baseline_description);
            let name = (name != baseline_name).then_some(name);
            if (!own_copy
                && name
                    .as_deref()
                    .is_none_or(|value| !blog_category_name_is_valid(value)))
                || name
                    .as_deref()
                    .is_some_and(|value| !blog_category_name_is_valid(value))
            {
                return Err(WritePathIssue::new(name_too_short_message));
            }
            BlogCategorySaveOperation::Update {
                category_id: category_id.to_string(),
                update: BlogCategoryUpdate {
                    locale,
                    name,
                    slug: slug.filter(|value| !own_copy || value != &loaded.slug),
                    description,
                },
            }
        }
        None => {
            let position = if input.position.trim().is_empty() {
                None
            } else {
                match input.position.trim().parse::<i32>() {
                    Ok(value) if value >= 0 => Some(value),
                    _ => return Err(WritePathIssue::new(invalid_position_message)),
                }
            };
            BlogCategorySaveOperation::Create(BlogCategoryDraft {
                locale,
                name,
                slug,
                description: optional_trimmed(input.description),
                parent_id: input.parent_id.and_then(optional_trimmed),
                position,
            })
        }
    };

    Ok(BlogCategorySaveCommand {
        operation,
        busy_key,
    })
}

/// Editable copy belongs to the exact target locale; the adapter renders the
/// source projection separately when this snapshot is empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryEditorCopy {
    pub name: String,
    pub slug: String,
    pub description: String,
}
pub fn category_editor_copy(category: &BlogCategoryDetail) -> CategoryEditorCopy {
    if category.locale == category.effective_locale {
        CategoryEditorCopy {
            name: category.name.clone(),
            slug: category.slug.clone(),
            description: category.description.clone().unwrap_or_default(),
        }
    } else {
        CategoryEditorCopy {
            name: String::new(),
            slug: String::new(),
            description: String::new(),
        }
    }
}

pub fn category_grid_columns(labels: [&str; 5]) -> Vec<rustok_grid::GridColumnDef> {
    ["name", "slug", "locale", "position", "actions"]
        .into_iter()
        .zip(labels)
        .map(|(id, title)| rustok_grid::GridColumnDef::new(id, title))
        .collect()
}

/// Preserve an assigned identity when option loading is unavailable or the
/// selected category is absent from a refreshed page. Display state cannot clear it.
pub fn post_category_options(
    items: &[crate::model::BlogCategoryOption],
    selected: &str,
    empty_label: String,
    current_label: String,
) -> Vec<rustok_ui_forms::FieldOption> {
    let mut options = vec![rustok_ui_forms::FieldOption::new("", empty_label)];
    options.extend(
        items
            .iter()
            .map(|item| rustok_ui_forms::FieldOption::new(item.id.clone(), item.name.clone())),
    );
    if !selected.is_empty() && !items.iter().any(|item| item.id == selected) {
        options.push(rustok_ui_forms::FieldOption::new(selected, current_label));
    }
    options
}
