use std::{collections::{BTreeMap, BTreeSet}, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::{
    FieldName, FieldPath, FilterExpr, IndexQuery, IndexQueryScope, LinkCardinality, LinkName,
    OrderDirection, Pagination, SchemaRef,
};

use super::{QueryValidationError, SchemaRegistry, SchemaRegistryError};

const ROOT_ALIAS: &str = "t0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QueryPlanFingerprint([u8; 32]);

impl QueryPlanFingerprint {
    pub fn as_bytes(&self) -> &[u8; 32] { &self.0 }
    pub fn to_hex(self) -> String { hex::encode(self.0) }
}

impl fmt::Display for QueryPlanFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedJoin {
    pub path: Vec<LinkName>,
    pub alias: String,
    pub source_alias: String,
    pub source_schema: SchemaRef,
    pub link: LinkName,
    pub target_schema: SchemaRef,
    pub source_fields: Vec<FieldName>,
    pub target_fields: Vec<FieldName>,
    pub cardinality: LinkCardinality,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedField {
    pub path: FieldPath,
    pub relation_alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOrder {
    pub field: PlannedField,
    pub direction: OrderDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableQueryPlan {
    pub scope: IndexQueryScope,
    pub root_schema: SchemaRef,
    pub root_alias: String,
    pub path_aliases: BTreeMap<Vec<LinkName>, String>,
    pub joins: Vec<PlannedJoin>,
    pub projection: Vec<PlannedField>,
    pub filter: Option<FilterExpr>,
    pub order_by: Vec<PlannedOrder>,
    pub pagination: Pagination,
    pub include_exact_count: bool,
}

impl ExecutableQueryPlan {
    pub fn relation_alias(&self, path: &FieldPath) -> Option<&str> {
        self.path_aliases.get(path.links()).map(String::as_str)
    }

    pub fn fingerprint(&self) -> Result<QueryPlanFingerprint, postcard::Error> {
        let bytes = postcard::to_stdvec(self)?;
        let mut hasher = Sha256::new();
        hasher.update(b"rustok-index-query-plan-v1");
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
        Ok(QueryPlanFingerprint(hasher.finalize().into()))
    }
}

#[derive(Debug, Error)]
pub enum QueryPlanError {
    #[error(transparent)]
    Validation(#[from] QueryValidationError),
    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),
}

impl SchemaRegistry {
    pub fn plan_query(&self, query: &IndexQuery) -> Result<ExecutableQueryPlan, QueryPlanError> {
        self.validate_query(query)?;

        let paths = collect_link_prefixes(query);
        let mut aliases = BTreeMap::from([(Vec::new(), ROOT_ALIAS.to_owned())]);
        for (index, path) in paths.iter().enumerate() {
            aliases.insert(path.clone(), format!("t{}", index + 1));
        }

        let mut schemas = BTreeMap::from([(Vec::new(), query.schema.clone())]);
        let mut joins = Vec::with_capacity(paths.len());
        for path in &paths {
            let parent = path[..path.len() - 1].to_vec();
            let source_schema = schemas.get(&parent).cloned().ok_or_else(|| {
                SchemaRegistryError::SchemaNotFound(query.schema.clone())
            })?;
            let registered = self.get(&source_schema).ok_or_else(|| {
                SchemaRegistryError::SchemaNotFound(source_schema.clone())
            })?;
            let link_name = path.last().cloned().ok_or_else(|| {
                SchemaRegistryError::SchemaNotFound(source_schema.clone())
            })?;
            let link = registered.schema.links.iter().find(|item| item.name == link_name)
                .ok_or_else(|| SchemaRegistryError::UnknownTargetSchema {
                    source_schema: source_schema.clone(),
                    link: link_name.clone(),
                    target: source_schema.clone(),
                })?;
            let target_schema = link.target_schema.clone();
            joins.push(PlannedJoin {
                path: path.clone(),
                alias: aliases[path].clone(),
                source_alias: aliases[&parent].clone(),
                source_schema,
                link: link.name.clone(),
                target_schema: target_schema.clone(),
                source_fields: link.source_fields.clone(),
                target_fields: link.target_fields.clone(),
                cardinality: link.cardinality,
            });
            schemas.insert(path.clone(), target_schema);
        }

        let projection = query.fields.iter().cloned().map(|path| PlannedField {
            relation_alias: aliases[path.links()].clone(),
            path,
        }).collect();
        let order_by = query.order_by.iter().cloned().map(|order| PlannedOrder {
            field: PlannedField {
                relation_alias: aliases[order.field.links()].clone(),
                path: order.field,
            },
            direction: order.direction,
        }).collect();

        Ok(ExecutableQueryPlan {
            scope: query.scope.clone(),
            root_schema: query.schema.clone(),
            root_alias: ROOT_ALIAS.to_owned(),
            path_aliases: aliases,
            joins,
            projection,
            filter: query.filter.clone(),
            order_by,
            pagination: query.pagination.clone(),
            include_exact_count: query.include_exact_count,
        })
    }
}

fn collect_link_prefixes(query: &IndexQuery) -> Vec<Vec<LinkName>> {
    let mut prefixes = BTreeSet::new();
    for path in query.referenced_paths() {
        for depth in 1..=path.links().len() {
            prefixes.insert(path.links()[..depth].to_vec());
        }
    }
    prefixes.into_iter().collect()
}
