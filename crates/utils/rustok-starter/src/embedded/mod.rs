use crate::model::StarterBlueprint;

const DEFAULT_STARTER_JSON: &str = include_str!("default_starter.json");

/// Returns the embedded canonical default starter blueprint.
pub fn default_starter() -> StarterBlueprint {
    serde_json::from_str(DEFAULT_STARTER_JSON)
        .expect("embedded default_starter.json must be valid according to StarterBlueprint schema")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_embedded_default_starter() {
        let blueprint = default_starter();
        assert_eq!(blueprint.id, "default-starter");
        assert_eq!(blueprint.schema_version, "1.0");
        assert_eq!(blueprint.locale, "ru");

        let content = blueprint.content;
        let taxonomy = content.taxonomy.expect("taxonomy should be present");
        assert_eq!(taxonomy.categories.len(), 3);
        assert_eq!(taxonomy.tags.len(), 6);

        let pages = content.pages.expect("pages should be present");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].slug, "home");

        let blog = content.blog.expect("blog should be present");
        assert_eq!(blog.posts.len(), 3);

        let forum = content.forum.expect("forum should be present");
        assert_eq!(forum.categories.len(), 4);
        assert_eq!(forum.topics.len(), 4);

        let navigation = content.navigation.expect("navigation should be present");
        assert_eq!(navigation.menus.len(), 1);
        assert_eq!(navigation.menus[0].items.len(), 3);
    }
}
