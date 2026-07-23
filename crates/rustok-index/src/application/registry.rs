use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fmt,
};

use petgraph::{
    graph::{DiGraph, NodeIndex},
    visit::EdgeRef,
};
use thiserror::Error;

use crate::domain::{
    DomainError, FieldName, IndexSchema, LinkName, SchemaFingerprint, SchemaIdentity, SchemaRef,
    SchemaVersion,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredSchema {
    pub schema: IndexSchema,
    pub fingerprint: SchemaFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationOutcome {
    Inserted {
        reference: SchemaRef,
        fingerprint: SchemaFingerprint,
    },
    Unchanged {
        reference: SchemaRef,
        fingerprint: SchemaFingerprint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPathStep {
    pub source: SchemaRef,
    pub link: LinkName,
    pub target: SchemaRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SchemaRegistryError {
    #[error(transparent)]
    InvalidSchema(#[from] DomainError),

    #[error("schema batch contains duplicate reference: {0}")]
    DuplicateBatchReference(SchemaRef),

    #[error(
        "schema {reference} already exists with fingerprint {existing}; incoming fingerprint is {incoming}"
    )]
    VersionConflict {
        reference: SchemaRef,
        existing: SchemaFingerprint,
        incoming: SchemaFingerprint,
    },

    #[error(
        "schema version must increase for {identity}: latest is {latest}, attempted {attempted}"
    )]
    NonMonotonicVersion {
        identity: SchemaIdentity,
        latest: SchemaVersion,
        attempted: SchemaVersion,
    },

    #[error("schema {source} link {link} targets unknown schema {target}")]
    UnknownTargetSchema {
        source: SchemaRef,
        link: LinkName,
        target: SchemaRef,
    },

    #[error("schema {source} link {link} targets unknown field {field} on {target}")]
    UnknownTargetField {
        source: SchemaRef,
        link: LinkName,
        target: SchemaRef,
        field: FieldName,
    },

    #[error("schema is not registered: {0}")]
    SchemaNotFound(SchemaRef),

    #[error("no registered link path exists from {from} to {to}")]
    LinkPathNotFound { from: SchemaRef, to: SchemaRef },
}

#[derive(Default)]
pub struct SchemaRegistry {
    schemas: BTreeMap<SchemaIdentity, BTreeMap<SchemaVersion, RegisteredSchema>>,
    graph: DiGraph<SchemaRef, LinkName>,
    nodes: BTreeMap<SchemaRef, NodeIndex>,
}

impl fmt::Debug for SchemaRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SchemaRegistry")
            .field("schema_count", &self.len())
            .field("link_count", &self.graph.edge_count())
            .finish()
    }
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.schemas.values().map(BTreeMap::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }

    pub fn get(&self, reference: &SchemaRef) -> Option<&RegisteredSchema> {
        self.schemas
            .get(&reference.identity())
            .and_then(|versions| versions.get(&reference.version))
    }

    pub fn latest(&self, identity: &SchemaIdentity) -> Option<&RegisteredSchema> {
        self.schemas
            .get(identity)
            .and_then(|versions| versions.last_key_value().map(|(_, schema)| schema))
    }

    pub fn iter(&self) -> impl Iterator<Item = &RegisteredSchema> {
        self.schemas.values().flat_map(BTreeMap::values)
    }

    pub fn register(
        &mut self,
        schema: IndexSchema,
    ) -> Result<RegistrationOutcome, SchemaRegistryError> {
        self.register_batch([schema])?
            .into_iter()
            .next()
            .ok_or_else(|| unreachable_registry_error())
    }

    /// Atomically validates and registers a group of schemas.
    ///
    /// Batch registration permits links to schemas declared later in the same
    /// batch while guaranteeing that no partial mutation occurs on error.
    pub fn register_batch(
        &mut self,
        schemas: impl IntoIterator<Item = IndexSchema>,
    ) -> Result<Vec<RegistrationOutcome>, SchemaRegistryError> {
        let mut incoming = BTreeMap::<SchemaRef, (IndexSchema, SchemaFingerprint)>::new();
        for schema in schemas {
            let fingerprint = schema.fingerprint()?;
            let reference = schema.reference.clone();
            if incoming
                .insert(reference.clone(), (schema, fingerprint))
                .is_some()
            {
                return Err(SchemaRegistryError::DuplicateBatchReference(reference));
            }
        }

        if incoming.is_empty() {
            return Ok(Vec::new());
        }

        for (reference, (_, incoming_fingerprint)) in &incoming {
            if let Some(existing) = self.get(reference) {
                if existing.fingerprint != *incoming_fingerprint {
                    return Err(SchemaRegistryError::VersionConflict {
                        reference: reference.clone(),
                        existing: existing.fingerprint,
                        incoming: *incoming_fingerprint,
                    });
                }
                continue;
            }

            if let Some((latest, _)) = self
                .schemas
                .get(&reference.identity())
                .and_then(BTreeMap::last_key_value)
            {
                if reference.version <= *latest {
                    return Err(SchemaRegistryError::NonMonotonicVersion {
                        identity: reference.identity(),
                        latest: *latest,
                        attempted: reference.version,
                    });
                }
            }
        }

        for (source_reference, (schema, _)) in &incoming {
            for link in &schema.links {
                let target = incoming
                    .get(&link.target_schema)
                    .map(|(schema, _)| schema)
                    .or_else(|| self.get(&link.target_schema).map(|entry| &entry.schema))
                    .ok_or_else(|| SchemaRegistryError::UnknownTargetSchema {
                        source: source_reference.clone(),
                        link: link.name.clone(),
                        target: link.target_schema.clone(),
                    })?;

                for target_field in &link.target_fields {
                    if !target
                        .fields
                        .iter()
                        .any(|field| field.name == *target_field)
                    {
                        return Err(SchemaRegistryError::UnknownTargetField {
                            source: source_reference.clone(),
                            link: link.name.clone(),
                            target: link.target_schema.clone(),
                            field: target_field.clone(),
                        });
                    }
                }
            }
        }

        let mut outcomes = Vec::with_capacity(incoming.len());
        for (reference, (schema, fingerprint)) in incoming {
            if self.get(&reference).is_some() {
                outcomes.push(RegistrationOutcome::Unchanged {
                    reference,
                    fingerprint,
                });
                continue;
            }

            self.schemas
                .entry(reference.identity())
                .or_default()
                .insert(
                    reference.version,
                    RegisteredSchema {
                        schema,
                        fingerprint,
                    },
                );
            outcomes.push(RegistrationOutcome::Inserted {
                reference,
                fingerprint,
            });
        }

        self.rebuild_graph();
        Ok(outcomes)
    }

    /// Resolves the shortest link path and breaks equal-length ties by link name
    /// and target schema identity.
    pub fn resolve_path(
        &self,
        from: &SchemaRef,
        to: &SchemaRef,
    ) -> Result<Vec<LinkPathStep>, SchemaRegistryError> {
        let start = self
            .nodes
            .get(from)
            .copied()
            .ok_or_else(|| SchemaRegistryError::SchemaNotFound(from.clone()))?;
        let goal = self
            .nodes
            .get(to)
            .copied()
            .ok_or_else(|| SchemaRegistryError::SchemaNotFound(to.clone()))?;

        if start == goal {
            return Ok(Vec::new());
        }

        let mut queue = VecDeque::from([start]);
        let mut visited = HashSet::from([start]);
        let mut previous = HashMap::<NodeIndex, (NodeIndex, LinkName)>::new();

        while let Some(node) = queue.pop_front() {
            let mut edges = self
                .graph
                .edges(node)
                .map(|edge| {
                    (
                        edge.weight().clone(),
                        self.graph[edge.target()].clone(),
                        edge.target(),
                    )
                })
                .collect::<Vec<_>>();
            edges.sort_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
            });

            for (link, _, target) in edges {
                if !visited.insert(target) {
                    continue;
                }
                previous.insert(target, (node, link));
                if target == goal {
                    return Ok(reconstruct_path(&self.graph, start, goal, &previous));
                }
                queue.push_back(target);
            }
        }

        Err(SchemaRegistryError::LinkPathNotFound {
            from: from.clone(),
            to: to.clone(),
        })
    }

    fn rebuild_graph(&mut self) {
        self.graph = DiGraph::new();
        self.nodes.clear();

        for registered in self.iter() {
            let reference = registered.schema.reference.clone();
            let node = self.graph.add_node(reference.clone());
            self.nodes.insert(reference, node);
        }

        let mut edges = self
            .iter()
            .flat_map(|registered| {
                registered.schema.links.iter().map(move |link| {
                    (
                        registered.schema.reference.clone(),
                        link.name.clone(),
                        link.target_schema.clone(),
                    )
                })
            })
            .collect::<Vec<_>>();
        edges.sort();

        for (source, link, target) in edges {
            if let (Some(source), Some(target)) =
                (self.nodes.get(&source), self.nodes.get(&target))
            {
                self.graph.add_edge(*source, *target, link);
            }
        }
    }
}

fn reconstruct_path(
    graph: &DiGraph<SchemaRef, LinkName>,
    start: NodeIndex,
    goal: NodeIndex,
    previous: &HashMap<NodeIndex, (NodeIndex, LinkName)>,
) -> Vec<LinkPathStep> {
    let mut reversed = Vec::new();
    let mut cursor = goal;

    while cursor != start {
        let (parent, link) = previous
            .get(&cursor)
            .expect("path predecessor must exist after successful traversal");
        reversed.push(LinkPathStep {
            source: graph[*parent].clone(),
            link: link.clone(),
            target: graph[cursor].clone(),
        });
        cursor = *parent;
    }

    reversed.reverse();
    reversed
}

fn unreachable_registry_error() -> SchemaRegistryError {
    SchemaRegistryError::InvalidSchema(DomainError::EmptySchema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        EntityName, FieldCardinality, IndexField, IndexLink, IndexValueType, LinkCardinality,
        LocaleMode, ModuleName,
    };

    fn reference(entity: &str, version: u32) -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new(entity).unwrap(),
            version: SchemaVersion::new(version),
        }
    }

    fn field(name: &str) -> IndexField {
        IndexField {
            name: FieldName::new(name).unwrap(),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: false,
        }
    }

    fn schema(entity: &str, version: u32) -> IndexSchema {
        IndexSchema {
            reference: reference(entity, version),
            locale_mode: LocaleMode::None,
            fields: vec![field("id")],
            links: Vec::new(),
        }
    }

    fn link(name: &str, target: SchemaRef) -> IndexLink {
        IndexLink {
            name: LinkName::new(name).unwrap(),
            source_fields: vec![FieldName::new("id").unwrap()],
            target_schema: target,
            target_fields: vec![FieldName::new("id").unwrap()],
            cardinality: LinkCardinality::Many,
        }
    }

    #[test]
    fn identical_registration_is_idempotent() {
        let mut registry = SchemaRegistry::new();
        let first = registry.register(schema("product", 1)).unwrap();
        let second = registry.register(schema("product", 1)).unwrap();

        assert!(matches!(first, RegistrationOutcome::Inserted { .. }));
        assert!(matches!(second, RegistrationOutcome::Unchanged { .. }));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn rejects_changed_contract_under_same_version() {
        let mut registry = SchemaRegistry::new();
        registry.register(schema("product", 1)).unwrap();
        let mut changed = schema("product", 1);
        changed.fields[0].filterable = false;

        assert!(matches!(
            registry.register(changed),
            Err(SchemaRegistryError::VersionConflict { .. })
        ));
    }

    #[test]
    fn rejects_non_monotonic_version() {
        let mut registry = SchemaRegistry::new();
        registry.register(schema("product", 2)).unwrap();

        assert!(matches!(
            registry.register(schema("product", 1)),
            Err(SchemaRegistryError::NonMonotonicVersion { .. })
        ));
    }

    #[test]
    fn batch_supports_forward_link_references() {
        let channel = schema("sales_channel", 1);
        let mut product = schema("product", 1);
        product.links.push(link("sales_channel", channel.reference.clone()));

        let mut registry = SchemaRegistry::new();
        registry.register_batch([product, channel]).unwrap();

        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn rejects_unknown_target_field_atomically() {
        let channel = schema("sales_channel", 1);
        let mut product = schema("product", 1);
        let mut invalid_link = link("sales_channel", channel.reference.clone());
        invalid_link.target_fields = vec![FieldName::new("missing").unwrap()];
        product.links.push(invalid_link);

        let mut registry = SchemaRegistry::new();
        assert!(matches!(
            registry.register_batch([product, channel]),
            Err(SchemaRegistryError::UnknownTargetField { .. })
        ));
        assert!(registry.is_empty());
    }

    #[test]
    fn path_resolution_is_shortest_and_deterministic() {
        let channel = schema("channel", 1);
        let mut category = schema("category", 1);
        category.links.push(link("channel", channel.reference.clone()));
        let mut collection = schema("collection", 1);
        collection.links.push(link("channel", channel.reference.clone()));
        let mut product = schema("product", 1);
        product.links.push(link("collection", collection.reference.clone()));
        product.links.push(link("category", category.reference.clone()));

        let mut registry = SchemaRegistry::new();
        registry
            .register_batch([
                product.clone(),
                collection,
                category.clone(),
                channel.clone(),
            ])
            .unwrap();

        let path = registry
            .resolve_path(&product.reference, &channel.reference)
            .unwrap();
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].link.as_str(), "category");
        assert_eq!(path[0].target, category.reference);
    }
}
