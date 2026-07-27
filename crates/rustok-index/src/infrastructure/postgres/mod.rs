mod mutation_store;
mod partition_admission;
mod schema_lease;
mod schema_registration;
mod secondary_index;

#[cfg(test)]
mod mutation_store_tests;
#[cfg(test)]
mod partition_admission_tests;
#[cfg(test)]
mod schema_lease_tests;
#[cfg(test)]
mod schema_registration_tests;
#[cfg(test)]
mod secondary_index_tests;

pub use mutation_store::{
    MutationApplyOutcome, MutationDelivery, MutationStorageError, PostgresMutationStore,
};
pub use partition_admission::{
    evaluate_partition_admission, PartitionAdmissionError, PartitionAdmissionOutcome,
    PartitionAdmissionPolicy, PartitionAdmissionReason, PartitionBaselineEvidence,
    PartitionEvidence, PartitionMeasurementCoverage, PartitionRelationPlan,
    PartitionShadowEvidence, PartitionShadowPlan, PartitionStrategy,
};
pub use schema_lease::{
    PostgresSchemaLeaseStore, SchemaApplicationLease, SchemaApplicationLeaseRequest,
    SchemaLeaseAcquireOutcome, SchemaLeaseError,
};
pub use schema_registration::{
    PersistedSchemaRegistrationOutcome, PostgresSchemaRegistrationStore, SchemaRegistrationError,
};
pub use secondary_index::{
    PostgresSecondaryIndexManager, SecondaryIndexClaimOutcome, SecondaryIndexError,
    SecondaryIndexExecutionOutcome, SecondaryIndexKind, SecondaryIndexLease,
    SecondaryIndexOperation, SecondaryIndexPlan, SecondaryIndexRequest, SecondaryIndexSpec,
};
