use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use rustok_core::{Error, Result};

use crate::engine::{SearchQuery, SearchResult, SearchResultItem};

#[derive(Debug, Clone, PartialEq)]
pub struct SearchSynonymRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub term: String,
    pub synonyms: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchStopWordRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub value: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchQueryRuleRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub query_text: String,
    pub query_normalized: String,
    pub rule_kind: String,
    pub document_id: Uuid,
    pub entity_type: String,
    pub source_module: String,
    pub title: String,
    pub pinned_position: u32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchDictionarySnapshot {
    pub synonyms: Vec<SearchSynonymRecord>,
    pub stop_words: Vec<SearchStopWordRecord>,
    pub query_rules: Vec<SearchQueryRuleRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchQueryTransform {
    pub original_query: String,
    pub effective_query: String,
    pub normalized_query: String,
    pub removed_stop_words: Vec<String>,
    pub applied_synonyms: Vec<String>,
}

pub struct SearchDictionaryService;

impl SearchDictionaryService {
    pub async fn snapshot(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<SearchDictionarySnapshot> {
        ensure_postgres(db)?;

        let synonyms = Self::load_synonyms(db, tenant_id).await?;
        let stop_words = Self::load_stop_words(db, tenant_id).await?;
        let query_rules = Self::load_query_rules(db, tenant_id).await?;

        Ok(SearchDictionarySnapshot {
            synonyms,
            stop_words,
            query_rules,
        })
    }

    pub async fn upsert_synonym(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        term: &str,
        synonyms: Vec<String>,
    ) -> Result<SearchSynonymRecord> {
        ensure_postgres(db)?;

        let normalized_term = normalize_token(term);
        if normalized_term.is_empty() {
            return Err(Error::Validation(
                "synonym term cannot be empty".to_string(),
            ));
        }

        let normalized_synonyms = normalize_unique_tokens(synonyms);
        if normalized_synonyms.is_empty() {
            return Err(Error::Validation(
                "synonym list must contain at least one value".to_string(),
            ));
        }

        let id = Uuid::new_v4();
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO search_synonyms (
                id,
                tenant_id,
                term,
                synonyms,
                updated_at
            )
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (tenant_id, term) DO UPDATE SET
                synonyms = EXCLUDED.synonyms,
                updated_at = NOW()
            RETURNING id, tenant_id, term, synonyms, updated_at
            "#,
            vec![
                id.into(),
                tenant_id.into(),
                normalized_term.into(),
                serde_json::json!(normalized_synonyms).into(),
            ],
        );

        let row = db
            .query_one(stmt)
            .await
            .map_err(Error::Database)?
            .ok_or_else(|| Error::NotFound("search synonym row".to_string()))?;

        map_synonym_row(row)
    }

    pub async fn delete_synonym(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        synonym_id: Uuid,
    ) -> Result<()> {
        ensure_postgres(db)?;

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM search_synonyms WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), synonym_id.into()],
        );
        db.execute(stmt).await.map_err(Error::Database)?;
        Ok(())
    }

    pub async fn add_stop_word(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        value: &str,
    ) -> Result<SearchStopWordRecord> {
        ensure_postgres(db)?;

        let normalized_value = normalize_token(value);
        if normalized_value.is_empty() {
            return Err(Error::Validation("stop word cannot be empty".to_string()));
        }

        let id = Uuid::new_v4();
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO search_stop_words (
                id,
                tenant_id,
                value,
                updated_at
            )
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (tenant_id, value) DO UPDATE SET
                updated_at = NOW()
            RETURNING id, tenant_id, value, updated_at
            "#,
            vec![id.into(), tenant_id.into(), normalized_value.into()],
        );

        let row = db
            .query_one(stmt)
            .await
            .map_err(Error::Database)?
            .ok_or_else(|| Error::NotFound("search stop word row".to_string()))?;

        map_stop_word_row(row)
    }

    pub async fn delete_stop_word(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        stop_word_id: Uuid,
    ) -> Result<()> {
        ensure_postgres(db)?;

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM search_stop_words WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), stop_word_id.into()],
        );
        db.execute(stmt).await.map_err(Error::Database)?;
        Ok(())
    }

    pub async fn upsert_pin_rule(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        query_text: &str,
        document_id: Uuid,
        pinned_position: u32,
    ) -> Result<SearchQueryRuleRecord> {
        ensure_postgres(db)?;

        let normalized_query = normalize_query_text(query_text);
        if normalized_query.is_empty() {
            return Err(Error::Validation(
                "query rule must target a non-empty query".to_string(),
            ));
        }

        let document_stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT document_id, entity_type, source_module, title
            FROM search_documents
            WHERE tenant_id = $1
              AND document_id = $2
            ORDER BY indexed_at DESC
            LIMIT 1
            "#,
            vec![tenant_id.into(), document_id.into()],
        );
        let document_row = db
            .query_one(document_stmt)
            .await
            .map_err(Error::Database)?
            .ok_or_else(|| Error::NotFound("search document for query rule".to_string()))?;

        let entity_type = document_row
            .try_get::<String>("", "entity_type")
            .map_err(Error::Database)?;
        let source_module = document_row
            .try_get::<String>("", "source_module")
            .map_err(Error::Database)?;
        let title = document_row
            .try_get::<String>("", "title")
            .map_err(Error::Database)?;

        let id = Uuid::new_v4();
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO search_query_rules (
                id,
                tenant_id,
                query_text,
                query_normalized,
                rule_kind,
                document_id,
                entity_type,
                source_module,
                title,
                pinned_position,
                updated_at
            )
            VALUES ($1, $2, $3, $4, 'pin_document', $5, $6, $7, $8, $9, NOW())
            ON CONFLICT (tenant_id, query_normalized, rule_kind, document_id) DO UPDATE SET
                query_text = EXCLUDED.query_text,
                entity_type = EXCLUDED.entity_type,
                source_module = EXCLUDED.source_module,
                title = EXCLUDED.title,
                pinned_position = EXCLUDED.pinned_position,
                updated_at = NOW()
            RETURNING id, tenant_id, query_text, query_normalized, rule_kind, document_id,
                      entity_type, source_module, title, pinned_position, updated_at
            "#,
            vec![
                id.into(),
                tenant_id.into(),
                query_text.trim().to_string().into(),
                normalized_query.into(),
                document_id.into(),
                entity_type.into(),
                source_module.into(),
                title.into(),
                (pinned_position.max(1) as i32).into(),
            ],
        );

        let row = db
            .query_one(stmt)
            .await
            .map_err(Error::Database)?
            .ok_or_else(|| Error::NotFound("search query rule row".to_string()))?;

        map_query_rule_row(row)
    }

    pub async fn delete_query_rule(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        query_rule_id: Uuid,
    ) -> Result<()> {
        ensure_postgres(db)?;

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM search_query_rules WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), query_rule_id.into()],
        );
        db.execute(stmt).await.map_err(Error::Database)?;
        Ok(())
    }

    pub async fn transform_query(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        raw_query: &str,
    ) -> Result<SearchQueryTransform> {
        ensure_postgres(db)?;

        let original_query = raw_query.trim().to_string();
        let normalized_query = normalize_query_text(&original_query);
        if normalized_query.is_empty() {
            return Ok(SearchQueryTransform {
                original_query,
                effective_query: String::new(),
                normalized_query: String::new(),
                removed_stop_words: Vec::new(),
                applied_synonyms: Vec::new(),
            });
        }

        let synonyms = Self::load_synonyms(db, tenant_id).await?;
        let stop_words = Self::load_stop_words(db, tenant_id).await?;
        let stop_word_set = stop_words
            .iter()
            .map(|record| record.value.clone())
            .collect::<BTreeSet<_>>();
        let synonym_map = build_synonym_map(&synonyms);

        let mut removed_stop_words = Vec::new();
        let mut applied_synonyms = Vec::new();
        let mut segments = Vec::new();

        for token in normalized_query.split_whitespace() {
            if stop_word_set.contains(token) {
                removed_stop_words.push(token.to_string());
                continue;
            }

            if let Some(group) = synonym_map.get(token) {
                if group.len() > 1 {
                    applied_synonyms.push(token.to_string());
                    segments.push(format!("({})", group.join(" OR ")));
                    continue;
                }
            }

            segments.push(token.to_string());
        }

        let effective_query = segments.join(" ");

        Ok(SearchQueryTransform {
            original_query,
            effective_query,
            normalized_query,
            removed_stop_words,
            applied_synonyms,
        })
    }

    pub async fn apply_query_rules(
        db: &DatabaseConnection,
        query: &SearchQuery,
        mut result: SearchResult,
    ) -> Result<SearchResult> {
        ensure_postgres(db)?;

        let tenant_id = query
            .tenant_id
            .ok_or_else(|| Error::Validation("query rules require tenant_id".to_string()))?;
        let normalized_query = normalize_query_text(&query.original_query);
        if normalized_query.is_empty() {
            return Ok(result);
        }

        let rules = Self::load_rules_for_query(db, tenant_id, &normalized_query).await?;
        if rules.is_empty() {
            return Ok(result);
        }

        let mut by_id = result
            .items
            .drain(..)
            .map(|item| (item.id, item))
            .collect::<BTreeMap<_, _>>();
        let mut pinned = Vec::new();

        for rule in &rules {
            if let Some(existing) = by_id.remove(&rule.document_id) {
                pinned.push((rule.pinned_position.max(1), existing));
                continue;
            }

            if let Some(item) = Self::load_pinned_item(db, query, rule.document_id).await? {
                pinned.push((rule.pinned_position.max(1), item));
            }
        }

        let mut ordered = by_id.into_values().collect::<Vec<_>>();
        for (pinned_position, item) in pinned.into_iter().rev() {
            let index = pinned_position.saturating_sub(1) as usize;
            ordered.insert(index.min(ordered.len()), item);
        }
        ordered.truncate(query.limit);
        result.items = ordered;
        Ok(result)
    }

    async fn load_pinned_item(
        db: &DatabaseConnection,
        query: &SearchQuery,
        document_id: Uuid,
    ) -> Result<Option<SearchResultItem>> {
        let tenant_id = query
            .tenant_id
            .ok_or_else(|| Error::Validation("query rules require tenant_id".to_string()))?;
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT document_id, entity_type, source_module, locale, status, is_public, title, payload
            FROM search_documents
            WHERE tenant_id = $1
              AND document_id = $2
            ORDER BY indexed_at DESC
            LIMIT 1
            "#,
            vec![tenant_id.into(), document_id.into()],
        );

        let row = db.query_one(stmt).await.map_err(Error::Database)?;
        row.filter(|row| pinned_item_matches_query(query, row))
            .map(map_pinned_item_row)
            .transpose()
    }

    async fn load_synonyms(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<SearchSynonymRecord>> {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT id, tenant_id, term, synonyms, updated_at
            FROM search_synonyms
            WHERE tenant_id = $1
            ORDER BY term ASC
            "#,
            vec![tenant_id.into()],
        );

        db.query_all(stmt)
            .await
            .map_err(Error::Database)?
            .into_iter()
            .map(map_synonym_row)
            .collect()
    }

    async fn load_stop_words(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<SearchStopWordRecord>> {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT id, tenant_id, value, updated_at
            FROM search_stop_words
            WHERE tenant_id = $1
            ORDER BY value ASC
            "#,
            vec![tenant_id.into()],
        );

        db.query_all(stmt)
            .await
            .map_err(Error::Database)?
            .into_iter()
            .map(map_stop_word_row)
            .collect()
    }

    async fn load_query_rules(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<SearchQueryRuleRecord>> {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT id, tenant_id, query_text, query_normalized, rule_kind, document_id,
                   entity_type, source_module, title, pinned_position, updated_at
            FROM search_query_rules
            WHERE tenant_id = $1
            ORDER BY query_normalized ASC, pinned_position ASC, updated_at DESC
            "#,
            vec![tenant_id.into()],
        );

        db.query_all(stmt)
            .await
            .map_err(Error::Database)?
            .into_iter()
            .map(map_query_rule_row)
            .collect()
    }

    async fn load_rules_for_query(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        normalized_query: &str,
    ) -> Result<Vec<SearchQueryRuleRecord>> {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT id, tenant_id, query_text, query_normalized, rule_kind, document_id,
                   entity_type, source_module, title, pinned_position, updated_at
            FROM search_query_rules
            WHERE tenant_id = $1
              AND query_normalized = $2
              AND rule_kind = 'pin_document'
            ORDER BY pinned_position ASC, updated_at DESC
            "#,
            vec![tenant_id.into(), normalized_query.to_string().into()],
        );

        db.query_all(stmt)
            .await
            .map_err(Error::Database)?
            .into_iter()
            .map(map_query_rule_row)
            .collect()
    }
}

fn ensure_postgres(db: &DatabaseConnection) -> Result<()> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(Error::External(
            "SearchDictionaryService requires PostgreSQL backend".to_string(),
        ));
    }

    Ok(())
}

fn map_synonym_row(row: QueryResult) -> Result<SearchSynonymRecord> {
    Ok(SearchSynonymRecord {
        id: row.try_get("", "id").map_err(Error::Database)?,
        tenant_id: row.try_get("", "tenant_id").map_err(Error::Database)?,
        term: row.try_get::<String>("", "term").map_err(Error::Database)?,
        synonyms: row
            .try_get::<serde_json::Value>("", "synonyms")
            .map_err(Error::Database)?
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
            .collect(),
        updated_at: row
            .try_get::<DateTime<Utc>>("", "updated_at")
            .map_err(Error::Database)?,
    })
}

fn map_stop_word_row(row: QueryResult) -> Result<SearchStopWordRecord> {
    Ok(SearchStopWordRecord {
        id: row.try_get("", "id").map_err(Error::Database)?,
        tenant_id: row.try_get("", "tenant_id").map_err(Error::Database)?,
        value: row
            .try_get::<String>("", "value")
            .map_err(Error::Database)?,
        updated_at: row
            .try_get::<DateTime<Utc>>("", "updated_at")
            .map_err(Error::Database)?,
    })
}

fn map_query_rule_row(row: QueryResult) -> Result<SearchQueryRuleRecord> {
    Ok(SearchQueryRuleRecord {
        id: row.try_get("", "id").map_err(Error::Database)?,
        tenant_id: row.try_get("", "tenant_id").map_err(Error::Database)?,
        query_text: row
            .try_get::<String>("", "query_text")
            .map_err(Error::Database)?,
        query_normalized: row
            .try_get::<String>("", "query_normalized")
            .map_err(Error::Database)?,
        rule_kind: row
            .try_get::<String>("", "rule_kind")
            .map_err(Error::Database)?,
        document_id: row.try_get("", "document_id").map_err(Error::Database)?,
        entity_type: row
            .try_get::<String>("", "entity_type")
            .map_err(Error::Database)?,
        source_module: row
            .try_get::<String>("", "source_module")
            .map_err(Error::Database)?,
        title: row
            .try_get::<String>("", "title")
            .map_err(Error::Database)?,
        pinned_position: row
            .try_get::<i32>("", "pinned_position")
            .map_err(Error::Database)?
            .max(1) as u32,
        updated_at: row
            .try_get::<DateTime<Utc>>("", "updated_at")
            .map_err(Error::Database)?,
    })
}

fn map_pinned_item_row(row: QueryResult) -> Result<SearchResultItem> {
    Ok(SearchResultItem {
        id: row.try_get("", "document_id").map_err(Error::Database)?,
        entity_type: row
            .try_get::<String>("", "entity_type")
            .map_err(Error::Database)?,
        source_module: row
            .try_get::<String>("", "source_module")
            .map_err(Error::Database)?,
        title: row
            .try_get::<String>("", "title")
            .map_err(Error::Database)?,
        snippet: Some("Pinned by query rule".to_string()),
        score: f64::MAX,
        locale: row
            .try_get::<String>("", "locale")
            .map(Some)
            .map_err(Error::Database)?,
        payload: row
            .try_get::<serde_json::Value>("", "payload")
            .map_err(Error::Database)?,
    })
}

fn pinned_item_matches_query(query: &SearchQuery, row: &QueryResult) -> bool {
    let entity_type = match row.try_get::<String>("", "entity_type") {
        Ok(value) => value,
        Err(_) => return false,
    };
    let source_module = match row.try_get::<String>("", "source_module") {
        Ok(value) => value,
        Err(_) => return false,
    };
    let locale = match row.try_get::<String>("", "locale") {
        Ok(value) => value,
        Err(_) => return false,
    };
    let status = match row.try_get::<String>("", "status") {
        Ok(value) => value,
        Err(_) => return false,
    };
    let is_public = match row.try_get::<bool>("", "is_public") {
        Ok(value) => value,
        Err(_) => return false,
    };

    if query.published_only && !is_public {
        return false;
    }
    if let Some(expected_locale) = query.locale.as_deref() {
        if !expected_locale.is_empty() && locale != expected_locale {
            return false;
        }
    }
    if !query.entity_types.is_empty() && !query.entity_types.contains(&entity_type) {
        return false;
    }
    if !query.source_modules.is_empty() && !query.source_modules.contains(&source_module) {
        return false;
    }
    if !query.statuses.is_empty() && !query.statuses.contains(&status) {
        return false;
    }

    true
}

fn build_synonym_map(records: &[SearchSynonymRecord]) -> BTreeMap<String, Vec<String>> {
    let mut map = BTreeMap::new();

    for record in records {
        let mut group = vec![record.term.clone()];
        group.extend(record.synonyms.iter().cloned());
        let group = normalize_unique_tokens(group);
        for token in &group {
            map.insert(token.clone(), group.clone());
        }
    }

    map
}

fn normalize_unique_tokens(values: Vec<String>) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for value in values {
        let token = normalize_token(&value);
        if !token.is_empty() {
            unique.insert(token);
        }
    }
    unique.into_iter().collect()
}

fn normalize_token(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':'))
        .collect()
}

pub fn normalize_query_text(value: &str) -> String {
    value
        .split_whitespace()
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{
        build_synonym_map, normalize_query_text, normalize_unique_tokens, SearchSynonymRecord,
    };
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn normalize_query_text_collapses_whitespace_and_case() {
        assert_eq!(
            normalize_query_text("  Summer   Sale "),
            "summer sale".to_string()
        );
    }

    #[test]
    fn normalize_unique_tokens_deduplicates_and_sanitizes() {
        assert_eq!(
            normalize_unique_tokens(vec![
                " Sale ".to_string(),
                "sale".to_string(),
                "SALE!".to_string()
            ]),
            vec!["sale".to_string()]
        );
    }

    #[test]
    fn build_synonym_map_exposes_group_for_all_members() {
        let record = SearchSynonymRecord {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            term: "tv".to_string(),
            synonyms: vec!["television".to_string(), "smart-tv".to_string()],
            updated_at: Utc::now(),
        };

        let map = build_synonym_map(&[record]);
        assert_eq!(
            map.get("television").cloned(),
            Some(vec![
                "smart-tv".to_string(),
                "television".to_string(),
                "tv".to_string()
            ])
        );
    }
}
