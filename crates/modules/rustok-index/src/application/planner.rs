use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::{
    FieldCardinality, FieldName, FieldPath, FilterExpr, IndexQuery, IndexQueryScope,
    IndexValueType, LinkCardinality, LinkName, OrderDirection, Pagination, SchemaRef,
};

use super::{
    AggregateOrderValidationError, QueryValidationError, SchemaRegistry, SchemaRegistryError,
};

const ROOT_ALIAS: &str = "t0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QueryPlanFingerprint([u8; 32]);

impl QueryPlanFingerprint {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for QueryPlanFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
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
    pub traverses_many: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedField {
    pub path: FieldPath,
    pub relation_alias: String,
    pub value_type: IndexValueType,
    pub cardinality: FieldCardinality,
    pub nullable: bool,
    pub traverses_many: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOrder {
    pub field: PlannedField,
    pub direction: OrderDirection,
}

/// One deterministic nested result group for projected fields sharing a terminal
/// relation path that crosses at least one many-cardinality link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedManyProjection {
    pub path: Vec<LinkName>,
    pub identity_paths: Vec<Vec<LinkName>>,
    pub fields: Vec<PlannedField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableQueryPlan {
    pub scope: IndexQueryScope,
    pub root_schema: SchemaRef,
    pub root_alias: String,
    pub path_aliases: BTreeMap<Vec<LinkName>, String>,
    pub joins: Vec<PlannedJoin>,
    pub referenced_fields: BTreeMap<FieldPath, PlannedField>,
    pub projection: Vec<PlannedField>,
    pub many_projections: Vec<PlannedManyProjection>,
    pub filter: Option<FilterExpr>,
    pub order_by: Vec<PlannedOrder>,
    pub pagination: Pagination,
    pub include_exact_count: bool,
}

impl ExecutableQueryPlan {
    pub fn relation_alias(&self, path: &FieldPath) -> Option<&str> {
        self.path_aliases.get(path.links()).map(String::as_str)
    }

    pub fn field(&self, path: &FieldPath) -> Option<&PlannedField> {
        self.referenced_fields.get(path)
    }

    pub(crate) fn join_for_path(&self, path: &[LinkName]) -> Option<&PlannedJoin> {
        self.joins.iter().find(|join| join.path.as_slice() == path)
    }

    pub(crate) fn outer_joins(&self) -> impl Iterator<Item = &PlannedJoin> {
        self.joins.iter().filter(|join| !join.traverses_many)
    }

    pub(crate) fn outer_projection(&self) -> impl Iterator<Item = &PlannedField> {
        self.projection.iter().filter(|field| !field.traverses_many)
    }

    pub fn fingerprint(&self) -> Result<QueryPlanFingerprint, postcard::Error> {
        let bytes = postcard::to_stdvec(self)?;
        let mut hasher = Sha256::new();
        hasher.update(b"rustok-index-query-plan-v4");
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
    AggregateValidation(AggregateOrderValidationError),
    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),
    #[error("validated query path has no relation alias: {0:?}")]
    ValidatedAliasMissing(FieldPath),
    #[error("validated query field {field} disappeared from schema {schema}")]
    ValidatedFieldMissing { schema: SchemaRef, field: FieldName },
}

impl SchemaRegistry {
    pub fn plan_query(&self, query: &IndexQuery) -> Result<ExecutableQueryPlan, QueryPlanError> {
        match self.validate_query_with_aggregate_ordering(query) {
            Ok(()) => {}
            Err(AggregateOrderValidationError::Query(error)) => {
                return Err(QueryPlanError::Validation(error));
            }
            Err(AggregateOrderValidationError::Registry(error)) => {
                return Err(QueryPlanError::Registry(error));
            }
            Err(error) => return Err(QueryPlanError::AggregateValidation(error)),
        }

        let paths = collect_link_prefixes(query);
        let mut aliases = BTreeMap::from([(Vec::new(), ROOT_ALIAS.to_owned())]);
        for (index, path) in paths.iter().enumerate() {
            aliases.insert(path.clone(), format!("t{}", index + 1));
        }

        let mut schemas = BTreeMap::from([(Vec::new(), query.schema.clone())]);
        let mut many_paths = BTreeMap::from([(Vec::new(), false)]);
        let mut joins = Vec::with_capacity(paths.len());
        for path in &paths {
            let parent = path[..path.len() - 1].to_vec();
            let source_schema = schemas
                .get(&parent)
                .cloned()
                .ok_or_else(|| SchemaRegistryError::SchemaNotFound(query.schema.clone()))?;
            let registered = self
                .get(&source_schema)
                .ok_or_else(|| SchemaRegistryError::SchemaNotFound(source_schema.clone()))?;
            let link_name = path
                .last()
                .cloned()
                .ok_or_else(|| SchemaRegistryError::SchemaNotFound(source_schema.clone()))?;
            let link = registered
                .schema
                .links
                .iter()
                .find(|item| item.name == link_name)
                .ok_or_else(|| SchemaRegistryError::UnknownTargetSchema {
                    source_schema: source_schema.clone(),
                    link: link_name.clone(),
                    target: source_schema.clone(),
                })?;
            let target_schema = link.target_schema.clone();
            let traverses_many = many_paths.get(&parent).copied().unwrap_or(false)
                || link.cardinality == LinkCardinality::Many;
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
                traverses_many,
            });
            schemas.insert(path.clone(), target_schema);
            many_paths.insert(path.clone(), traverses_many);
        }

        let referenced_paths = query
            .referenced_paths()
            .into_iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut referenced_fields = BTreeMap::new();
        for path in referenced_paths {
            let link_path = path.links().to_vec();
            let schema = schemas
                .get(&link_path)
                .cloned()
                .ok_or_else(|| QueryPlanError::ValidatedAliasMissing(path.clone()))?;
            let registered = self
                .get(&schema)
                .ok_or_else(|| SchemaRegistryError::SchemaNotFound(schema.clone()))?;
            let field = registered
                .schema
                .fields
                .iter()
                .find(|field| field.name == *path.field())
                .ok_or_else(|| QueryPlanError::ValidatedFieldMissing {
                    schema: schema.clone(),
                    field: path.field().clone(),
                })?;
            let relation_alias = aliases
                .get(&link_path)
                .cloned()
                .ok_or_else(|| QueryPlanError::ValidatedAliasMissing(path.clone()))?;
            let traverses_many = many_paths
                .get(&link_path)
                .copied()
                .ok_or_else(|| QueryPlanError::ValidatedAliasMissing(path.clone()))?;
            referenced_fields.insert(
                path.clone(),
                PlannedField {
                    path,
                    relation_alias,
                    value_type: field.value_type,
                    cardinality: field.cardinality,
                    nullable: field.nullable,
                    traverses_many,
                },
            );
        }

        let projection = query
            .fields
            .iter()
            .map(|path| planned_field(&referenced_fields, path))
            .collect::<Result<Vec<_>, _>>()?;
        let many_projections = derive_many_projections(&projection);
        let order_by = query
            .order_by
            .iter()
            .map(|order| {
                let mut field = planned_field(&referenced_fields, &order.field)?;
                if order.direction.aggregate().is_some() {
                    field.nullable = true;
                }
                Ok(PlannedOrder {
                    field,
                    direction: order.direction,
                })
            })
            .collect::<Result<Vec<_>, QueryPlanError>>()?;

        Ok(ExecutableQueryPlan {
            scope: query.scope.clone(),
            root_schema: query.schema.clone(),
            root_alias: ROOT_ALIAS.to_owned(),
            path_aliases: aliases,
            joins,
            referenced_fields,
            projection,
            many_projections,
            filter: query.filter.clone(),
            order_by,
            pagination: query.pagination.clone(),
            include_exact_count: query.include_exact_count,
        })
    }
}

fn planned_field(
    referenced_fields: &BTreeMap<FieldPath, PlannedField>,
    path: &FieldPath,
) -> Result<PlannedField, QueryPlanError> {
    referenced_fields
        .get(path)
        .cloned()
        .ok_or_else(|| QueryPlanError::ValidatedAliasMissing(path.clone()))
}

pub(crate) fn derive_many_projections(projection: &[PlannedField]) -> Vec<PlannedManyProjection> {
    let mut group_indexes = BTreeMap::<Vec<LinkName>, usize>::new();
    let mut groups = Vec::<PlannedManyProjection>::new();

    for field in projection.iter().filter(|field| field.traverses_many) {
        let path = field.path.links().to_vec();
        if let Some(index) = group_indexes.get(&path).copied() {
            groups[index].fields.push(field.clone());
            continue;
        }

        let identity_paths = (1..=path.len())
            .map(|depth| path[..depth].to_vec())
            .collect();
        group_indexes.insert(path.clone(), groups.len());
        groups.push(PlannedManyProjection {
            path,
            identity_paths,
            fields: vec![field.clone()],
        });
    }

    groups
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
