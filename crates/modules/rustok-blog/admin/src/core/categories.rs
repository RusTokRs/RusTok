use crate::model::{BlogCategoryDetail, BlogCategoryDraft, BlogCategoryUpdate};
use rustok_api::{Patch, WritePathIssue};
use rustok_ui_core::{ui_busy_key, ui_busy_key_with_id};

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
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }
    category.name.to_ascii_lowercase().contains(&needle)
        || category.slug.to_ascii_lowercase().contains(&needle)
        || category.description.to_ascii_lowercase().contains(&needle)
}

/// Category names must be at least two characters, mirroring the Next admin
/// form contract; the owner additionally enforces its own length cap.
pub const MIN_BLOG_CATEGORY_NAME_LENGTH: usize = 2;

pub fn blog_category_name_is_valid(name: &str) -> bool {
    name.trim().chars().count() >= MIN_BLOG_CATEGORY_NAME_LENGTH
}

/// The update payload sends the description only when it changed relative to
/// the loaded row: an unchanged description is a keep, an emptied one is a
/// clear, and a new text is a set. This mirrors the owner's Patch semantics
/// and keeps "clear" distinguishable from "not supplied".
pub fn blog_category_description_patch(current: &str, loaded: Option<&str>) -> Patch<String> {
    let trimmed = current.trim();
    match loaded {
        Some(existing) if trimmed == existing => Patch::Keep,
        _ if trimmed.is_empty() => Patch::Clear,
        _ => Patch::Set(trimmed.to_string()),
    }
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
    /// Description currently loaded in the editor for the edited row; the
    /// update payload keeps an unchanged description instead of re-sending it.
    pub loaded_description: Option<&'a str>,
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
/// mirrors the Next admin form contract: the name needs at least two
/// characters and the create-only position must parse as a non-negative
/// integer (the owner additionally clamps the position to the sibling count).
pub fn prepare_blog_category_save_command(
    input: BlogCategoryFormInput<'_>,
    name_too_short_message: String,
    invalid_position_message: String,
) -> Result<BlogCategorySaveCommand, WritePathIssue> {
    let name = input.name.trim().to_string();
    if !blog_category_name_is_valid(&name) {
        return Err(WritePathIssue::new(name_too_short_message));
    }
    let slug = optional_trimmed(input.slug);
    let locale = input.locale.trim().to_string();

    let busy_key = busy_key_for_category_save(input.editing_category_id);
    let operation = match input.editing_category_id {
        Some(category_id) => BlogCategorySaveOperation::Update {
            category_id: category_id.to_string(),
            update: BlogCategoryUpdate {
                locale,
                name: Some(name),
                slug,
                description: blog_category_description_patch(
                    input.description,
                    input.loaded_description,
                ),
            },
        },
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
