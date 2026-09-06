use parking_lot::RwLock;
use rhai::{CustomType, Dynamic, TypeBuilder};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct EntityProxy {
    id: String,
    entity_type: String,
    state: Arc<RwLock<EntityState>>,
}

#[derive(Debug)]
struct EntityState {
    data: HashMap<String, Dynamic>,
    changes: HashMap<String, Dynamic>,
}

impl EntityProxy {
    pub fn new(
        id: impl Into<String>,
        entity_type: impl Into<String>,
        data: HashMap<String, Dynamic>,
    ) -> Self {
        Self {
            id: id.into(),
            entity_type: entity_type.into(),
            state: Arc::new(RwLock::new(EntityState {
                data,
                changes: HashMap::new(),
            })),
        }
    }

    pub fn empty(entity_type: impl Into<String>) -> Self {
        Self::new("", entity_type, HashMap::new())
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn entity_type(&self) -> &str {
        &self.entity_type
    }

    pub fn get(&self, field: &str) -> Dynamic {
        let state = self.state.read();
        state
            .changes
            .get(field)
            .or_else(|| state.data.get(field))
            .cloned()
            .unwrap_or(Dynamic::UNIT)
    }

    pub fn set(&mut self, field: &str, value: Dynamic) {
        let mut state = self.state.write();
        state.changes.insert(field.to_string(), value);
    }

    pub fn is_changed(&self, field: &str) -> bool {
        let state = self.state.read();
        state.changes.contains_key(field)
    }

    pub fn changes(&self) -> HashMap<String, Dynamic> {
        let state = self.state.read();
        state.changes.clone()
    }

    pub fn take_changes(&self) -> HashMap<String, Dynamic> {
        let mut state = self.state.write();
        std::mem::take(&mut state.changes)
    }

    pub fn original(&self) -> HashMap<String, Dynamic> {
        let state = self.state.read();
        state.data.clone()
    }

    pub fn snapshot(&self) -> HashMap<String, Dynamic> {
        let state = self.state.read();
        let mut result = state.data.clone();
        for (key, value) in &state.changes {
            result.insert(key.clone(), value.clone());
        }
        result
    }

    pub fn has_changes(&self) -> bool {
        let state = self.state.read();
        !state.changes.is_empty()
    }

    pub fn clear_changes(&self) {
        let mut state = self.state.write();
        state.changes.clear();
    }

    pub fn commit_changes(&self) {
        let mut state = self.state.write();
        let changes = std::mem::take(&mut state.changes);
        for (key, value) in changes {
            state.data.insert(key, value);
        }
    }
}

impl CustomType for EntityProxy {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Entity")
            .with_get("id", |entity: &mut EntityProxy| entity.id.clone())
            .with_get("type", |entity: &mut EntityProxy| {
                entity.entity_type.clone()
            })
            .with_indexer_get(|entity: &mut EntityProxy, key: &str| entity.get(key))
            .with_indexer_set(|entity: &mut EntityProxy, key: &str, val: Dynamic| {
                entity.set(key, val);
            })
            .with_fn("is_changed", |entity: &mut EntityProxy, field: &str| {
                entity.is_changed(field)
            })
            .with_fn("has_changes", |entity: &mut EntityProxy| {
                entity.has_changes()
            })
            .with_fn("snapshot", |entity: &mut EntityProxy| entity.snapshot());
    }
}

pub fn register_entity_proxy(engine: &mut rhai::Engine) {
    engine.build_type::<EntityProxy>();
}
