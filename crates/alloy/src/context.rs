use chrono::{DateTime, Utc};
use rhai::Map;
#[cfg(test)]
use rhai::Scope;
#[cfg(test)]
use rustok_sandbox::rhai::RhaiScopeProvider;
use uuid::Uuid;

use crate::model::EntityProxy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPhase {
    Before,
    After,
    OnCommit,
    Manual,
    Scheduled,
}

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub execution_id: Uuid,
    pub phase: ExecutionPhase,
    pub timestamp: DateTime<Utc>,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub entity_proxy: Option<EntityProxy>,
    pub entity_before_proxy: Option<EntityProxy>,
    pub params: Map,
    pub call_depth: usize,
}

impl ExecutionContext {
    pub fn new(phase: ExecutionPhase) -> Self {
        Self {
            execution_id: Uuid::new_v4(),
            phase,
            timestamp: Utc::now(),
            user_id: None,
            tenant_id: None,
            entity_proxy: None,
            entity_before_proxy: None,
            params: Map::new(),
            call_depth: 0,
        }
    }

    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    pub fn with_entity_proxy(mut self, proxy: EntityProxy) -> Self {
        self.entity_proxy = Some(proxy);
        self
    }

    pub fn with_entity_before_proxy(mut self, proxy: EntityProxy) -> Self {
        self.entity_before_proxy = Some(proxy);
        self
    }

    pub fn with_params(mut self, params: Map) -> Self {
        self.params = params;
        self
    }

    pub fn child(&self) -> Self {
        Self {
            execution_id: self.execution_id,
            phase: self.phase,
            timestamp: Utc::now(),
            user_id: self.user_id.clone(),
            tenant_id: self.tenant_id.clone(),
            entity_proxy: None,
            entity_before_proxy: None,
            params: Map::new(),
            call_depth: self.call_depth + 1,
        }
    }

    #[cfg(test)]
    pub fn to_scope(&self) -> Scope<'static> {
        let mut scope = Scope::new();

        scope.push_constant("EXECUTION_ID", self.execution_id.to_string());
        scope.push_constant("PHASE", format!("{:?}", self.phase));
        scope.push_constant("TIMESTAMP", self.timestamp.to_rfc3339());

        if let Some(ref user_id) = self.user_id {
            scope.push_constant("USER_ID", user_id.clone());
        }
        if let Some(ref tenant_id) = self.tenant_id {
            scope.push_constant("TENANT_ID", tenant_id.clone());
        }

        if let Some(ref proxy) = self.entity_proxy {
            match self.phase {
                ExecutionPhase::Before => scope.push("entity", proxy.clone()),
                _ => scope.push_constant("entity", proxy.clone()),
            };
        }

        if let Some(ref proxy) = self.entity_before_proxy {
            scope.push_constant("entity_before", proxy.clone());
        }

        scope.push_constant("params", self.params.clone());

        scope
    }
}

#[cfg(test)]
impl RhaiScopeProvider for ExecutionContext {
    fn rhai_scope(&self) -> Scope<'static> {
        self.to_scope()
    }
}
