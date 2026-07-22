mod m20260721_000001_create_groups;
mod m20260721_000002_create_group_governance;
mod m20260721_000003_enforce_group_language_agnostic_storage;
mod m20260721_000004_create_group_invitations;
mod m20260721_000005_create_group_domain_events;
mod m20260722_000006_create_group_membership_applications;
mod m20260722_000007_create_group_membership_policy_revisions;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260721_000001_create_groups::Migration),
        Box::new(m20260721_000002_create_group_governance::Migration),
        Box::new(m20260721_000003_enforce_group_language_agnostic_storage::Migration),
        Box::new(m20260721_000004_create_group_invitations::Migration),
        Box::new(m20260721_000005_create_group_domain_events::Migration),
        Box::new(m20260722_000006_create_group_membership_applications::Migration),
        Box::new(m20260722_000007_create_group_membership_policy_revisions::Migration),
    ]
}
