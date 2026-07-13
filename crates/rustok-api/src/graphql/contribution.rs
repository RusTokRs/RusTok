/// Manifest-backed description of a typed GraphQL contribution.
///
/// The deployment build script consumes these symbolic paths to generate
/// compile-time `MergedObject`/`MergedSubscription` composition. The host
/// therefore never maps capability slugs to GraphQL types by hand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphqlContributionDescriptor {
    pub query: Option<&'static str>,
    pub mutation: Option<&'static str>,
    pub subscription: Option<&'static str>,
    pub runtime_data_factory: Option<&'static str>,
}

/// Neutral host values available to a manifest-generated GraphQL runtime-data
/// factory. Values beyond the database are carried through the host's typed
/// shared-value store, so foundation does not depend on capability crates.
#[cfg(feature = "server")]
#[derive(Clone)]
pub struct GraphqlRuntimeInputs {
    host: crate::HostRuntimeContext,
}

#[cfg(feature = "server")]
impl GraphqlRuntimeInputs {
    pub fn new(host: crate::HostRuntimeContext) -> Self {
        Self { host }
    }

    pub fn host(&self) -> &crate::HostRuntimeContext {
        &self.host
    }

    pub fn db_clone(&self) -> sea_orm::DatabaseConnection {
        self.host.db_clone()
    }

    pub fn shared_get<T>(&self) -> Option<T>
    where
        T: 'static + Send + Sync + Clone,
    {
        self.host.shared_get::<T>()
    }
}

impl GraphqlContributionDescriptor {
    pub const fn new(
        query: Option<&'static str>,
        mutation: Option<&'static str>,
        subscription: Option<&'static str>,
        runtime_data_factory: Option<&'static str>,
    ) -> Self {
        Self {
            query,
            mutation,
            subscription,
            runtime_data_factory,
        }
    }
}
