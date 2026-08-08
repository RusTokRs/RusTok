use std::{fmt, sync::Arc};

use async_trait::async_trait;

use crate::domain::{IndexQuery, LocalizedEntityQuery};

use super::{IndexQueryExecutionError, IndexQueryPage, IndexQueryPort};

/// Cloneable transport-neutral Index query capability published by executable hosts.
///
/// Construction is crate-owned so production hosts cannot wrap an arbitrary adapter while
/// claiming the canonical Index runtime. Consumers depend only on [`IndexQueryPort`] and do
/// not receive the PostgreSQL connection or mutable schema-registry internals.
#[derive(Clone)]
pub struct SharedIndexQueryRuntime {
    port: Arc<dyn IndexQueryPort>,
}

impl SharedIndexQueryRuntime {
    pub(crate) fn new(port: Arc<dyn IndexQueryPort>) -> Self {
        Self { port }
    }

    pub fn shared_port(&self) -> Arc<dyn IndexQueryPort> {
        self.port.clone()
    }
}

impl fmt::Debug for SharedIndexQueryRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedIndexQueryRuntime")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl IndexQueryPort for SharedIndexQueryRuntime {
    async fn execute_query(
        &self,
        query: IndexQuery,
    ) -> Result<IndexQueryPage, IndexQueryExecutionError> {
        self.port.execute_query(query).await
    }

    async fn execute_localized_query(
        &self,
        query: LocalizedEntityQuery,
    ) -> Result<IndexQueryPage, IndexQueryExecutionError> {
        self.port.execute_localized_query(query).await
    }
}
