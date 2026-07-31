use sea_orm::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForumStorefrontDocumentFilters {
    pub author_ids: Vec<Uuid>,
}

impl ForumStorefrontDocumentFilters {
    pub fn is_empty(&self) -> bool {
        self.author_ids.is_empty()
    }
}

pub(crate) fn forum_document_filter_sql(
    filters: &ForumStorefrontDocumentFilters,
    bound_values: &mut Vec<Value>,
    next_param: &mut usize,
) -> Option<String> {
    if filters.author_ids.is_empty() {
        return None;
    }

    let author_params = filters
        .author_ids
        .iter()
        .map(|author_id| {
            let placeholder = format!("${}", *next_param);
            bound_values.push(author_id.to_string().into());
            *next_param += 1;
            placeholder
        })
        .collect::<Vec<_>>()
        .join(", ");

    Some(format!(
        "(
            source_module = 'forum'
            AND entity_type IN ('forum_topic', 'forum_reply')
            AND facets ->> 'author_id' IN ({author_params})
        )"
    ))
}

#[cfg(test)]
mod tests {
    use sea_orm::Value;
    use uuid::Uuid;

    use super::{
        ForumStorefrontDocumentFilters, forum_document_filter_sql,
    };

    #[test]
    fn empty_filter_does_not_change_search_scope() {
        let mut values = Vec::<Value>::new();
        let mut next_param = 4;

        assert!(
            forum_document_filter_sql(
                &ForumStorefrontDocumentFilters::default(),
                &mut values,
                &mut next_param,
            )
            .is_none()
        );
        assert!(values.is_empty());
        assert_eq!(next_param, 4);
    }

    #[test]
    fn author_filter_matches_only_forum_topics_and_replies() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let mut values = Vec::<Value>::new();
        let mut next_param = 7;
        let sql = forum_document_filter_sql(
            &ForumStorefrontDocumentFilters {
                author_ids: vec![first, second],
            },
            &mut values,
            &mut next_param,
        )
        .expect("author filter SQL");

        assert!(sql.contains("source_module = 'forum'"));
        assert!(sql.contains("entity_type IN ('forum_topic', 'forum_reply')"));
        assert!(sql.contains("facets ->> 'author_id' IN ($7, $8)"));
        assert_eq!(values.len(), 2);
        assert_eq!(next_param, 9);
    }
}
