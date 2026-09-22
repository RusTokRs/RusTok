use std::collections::HashMap;

use rhai::Dynamic;

use crate::model::EntityProxy;

pub trait ScriptableEntity: Send + Sync {
    fn entity_type(&self) -> &'static str;
    fn id(&self) -> String;
    fn to_dynamic_map(&self) -> HashMap<String, Dynamic>;
    fn apply_changes(&mut self, changes: HashMap<String, Dynamic>);

    fn to_entity_proxy(&self) -> EntityProxy {
        EntityProxy::new(self.id(), self.entity_type(), self.to_dynamic_map())
    }
}
