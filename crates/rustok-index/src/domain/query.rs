use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DomainError, FieldPath, IndexValue, LocaleKey, SchemaRef};

const MAX_CURSOR_PAGE_SIZE: u32 = 500;
const MAX_OFFSET_LIMIT: u32 = 100;
const MAX_OFFSET_DEPTH: u64 = 10_000;
const MAX_SELECTED_FIELDS: usize = 100;
const MAX_ORDER_EXPRESSIONS: usize = 8;
const MAX_FILTER_NODES: usize = 128;
const MAX_LINK_PATH_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterExpr {
    And(Vec<FilterExpr>),
    Or(Vec<FilterExpr>),
    Not(Box<FilterExpr>),
    Eq(FieldPath, IndexValue),
    Ne(FieldPath, IndexValue),
    In(FieldPath, Vec<IndexValue>),
    Gt(FieldPath, IndexValue),
    Gte(FieldPath, IndexValue),
    Lt(FieldPath, IndexValue),
    Lte(FieldPath, IndexValue),
    Contains(FieldPath, IndexValue),
    IsNull(FieldPath, bool),
}

impl FilterExpr {
    pub fn field_paths(&self, output: &mut Vec<&FieldPath>) {
        match self {
            Self::And(filters) | Self::Or(filters) => {
                for filter in filters {
                    filter.field_paths(output);
                }
            }
            Self::Not(filter) => filter.field_paths(output),
            Self::Eq(path, _)
            | Self::Ne(path, _)
            | Self::In(path, _)
            | Self::Gt(path, _)
            | Self::Gte(path, _)
            | Self::Lt(path, _)
            | Self::Lte(path, _)
            | Self::Contains(path, _)
            | Self::IsNull(path, _) => output.push(path),
        }
    }

    pub fn node_count(&self) -> usize {
        match self {
            Self::And(filters) | Self::Or(filters) => {
                1 + filters.iter().map(Self::node_count).sum::<usize>()
            }
            Self::Not(filter) => 1 + filter.node_count(),
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderExpr {
    pub field: FieldPath,
    pub direction: OrderDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexQueryScope {
    pub tenant_id: Uuid,
    pub locale: Option<LocaleKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pagination {
    Cursor {
        first: u32,
        after: Option<String>,
    },
    Offset {
        limit: u32,
        offset: u64,
    },
}

impl Pagination {
    pub fn validate(&self) -> Result<(), DomainError> {
        match self {
            Self::Cursor { first, .. } => {
                if *first == 0 {
                    Err(DomainError::EmptyPage)
                } else if *first > MAX_CURSOR_PAGE_SIZE {
                    Err(DomainError::PageTooLarge)
                } else {
                    Ok(())
                }
            }
            Self::Offset { limit, offset } => {
                if *limit == 0 {
                    Err(DomainError::EmptyPage)
                } else if *limit > MAX_OFFSET_LIMIT {
                    Err(DomainError::OffsetLimitExceeded)
                } else if *offset > MAX_OFFSET_DEPTH {
                    Err(DomainError::OffsetTooDeep)
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexQuery {
    pub scope: IndexQueryScope,
    pub schema: SchemaRef,
    pub fields: Vec<FieldPath>,
    pub filter: Option<FilterExpr>,
    pub order_by: Vec<OrderExpr>,
    pub pagination: Pagination,
    pub include_exact_count: bool,
}

impl IndexQuery {
    pub fn validate_shape(&self) -> Result<(), DomainError> {
        if self.fields.is_empty() {
            return Err(DomainError::EmptySelection);
        }
        if self.fields.len() > MAX_SELECTED_FIELDS {
            return Err(DomainError::TooManySelectedFields);
        }
        if self.order_by.len() > MAX_ORDER_EXPRESSIONS {
            return Err(DomainError::TooManyOrderExpressions);
        }
        if self
            .filter
            .as_ref()
            .is_some_and(|filter| filter.node_count() > MAX_FILTER_NODES)
        {
            return Err(DomainError::FilterTooComplex);
        }
        if self
            .referenced_paths()
            .into_iter()
            .any(|path| path.depth() > MAX_LINK_PATH_DEPTH)
        {
            return Err(DomainError::LinkPathTooDeep);
        }

        self.pagination.validate()
    }

    pub fn referenced_paths(&self) -> Vec<&FieldPath> {
        let mut paths = self.fields.iter().collect::<Vec<_>>();
        if let Some(filter) = &self.filter {
            filter.field_paths(&mut paths);
        }
        paths.extend(self.order_by.iter().map(|order| &order.field));
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EntityName, FieldName, LinkName, ModuleName, SchemaVersion};

    fn schema() -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new("product").unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn scope() -> IndexQueryScope {
        IndexQueryScope {
            tenant_id: Uuid::new_v4(),
            locale: Some(LocaleKey::new("en-US").unwrap()),
        }
    }

    fn base_query() -> IndexQuery {
        IndexQuery {
            scope: scope(),
            schema: schema(),
            fields: vec![FieldPath::new(FieldName::new("id").unwrap())],
            filter: None,
            order_by: Vec::new(),
            pagination: Pagination::Cursor {
                first: 20,
                after: None,
            },
            include_exact_count: false,
        }
    }

    #[test]
    fn rejects_empty_query_selection() {
        let mut query = base_query();
        query.fields.clear();
        assert_eq!(query.validate_shape(), Err(DomainError::EmptySelection));
    }

    #[test]
    fn rejects_unbounded_pages_and_deep_offsets() {
        assert_eq!(
            Pagination::Cursor {
                first: 501,
                after: None,
            }
            .validate(),
            Err(DomainError::PageTooLarge)
        );
        assert_eq!(
            Pagination::Offset {
                limit: 20,
                offset: 10_001,
            }
            .validate(),
            Err(DomainError::OffsetTooDeep)
        );
    }

    #[test]
    fn rejects_excessive_link_depth() {
        let mut query = base_query();
        query.fields = vec![FieldPath::linked(
            (0..=MAX_LINK_PATH_DEPTH)
                .map(|index| LinkName::new(format!("link_{index}")).unwrap()),
            FieldName::new("id").unwrap(),
        )];

        assert_eq!(query.validate_shape(), Err(DomainError::LinkPathTooDeep));
    }

    #[test]
    fn accepts_link_aware_filter_shape() {
        let mut query = base_query();
        query.filter = Some(FilterExpr::Eq(
            FieldPath::linked(
                [LinkName::new("sales_channel").unwrap()],
                FieldName::new("id").unwrap(),
            ),
            IndexValue::String("channel-eu".to_owned()),
        ));
        query.include_exact_count = true;

        assert!(query.validate_shape().is_ok());
        assert_eq!(query.referenced_paths().len(), 2);
    }
}
