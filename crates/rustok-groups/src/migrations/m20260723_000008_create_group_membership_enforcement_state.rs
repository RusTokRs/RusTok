use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(GroupMemberships::Table)
                    .add_column(
                        ColumnDef::new(GroupMemberships::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_group_memberships_tenant_id")
                    .table(GroupMemberships::Table)
                    .col(GroupMemberships::TenantId)
                    .col(GroupMemberships::Id)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(GroupMembershipEnforcements::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::MembershipId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::GroupId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::State)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::ReasonCode)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::SourceKind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::EffectiveFrom)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::EffectiveUntil)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::RestoreStatus)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::ModerationDecisionId).uuid(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::ModerationDecisionHash)
                            .string_len(64),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::ActorKind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::ActorId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::RevokedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(GroupMembershipEnforcements::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .check(Expr::cust("state IN ('suspended')"))
                    .check(Expr::cust(
                        "source_kind IN ('direct_local', 'moderation_decision')",
                    ))
                    .check(Expr::cust(
                        "restore_status IN ('active', 'pending', 'invited', 'left')",
                    ))
                    .check(Expr::cust("actor_kind IN ('user', 'service', 'system')"))
                    .check(Expr::cust("revision >= 1"))
                    .check(Expr::cust(
                        "effective_until IS NULL OR effective_until > effective_from",
                    ))
                    .check(Expr::cust(
                        "(source_kind = 'moderation_decision' AND moderation_decision_id IS NOT NULL AND moderation_decision_hash IS NOT NULL) OR (source_kind = 'direct_local' AND moderation_decision_id IS NULL AND moderation_decision_hash IS NULL)",
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_group_membership_enforcements_tenant_membership")
                            .from(
                                GroupMembershipEnforcements::Table,
                                GroupMembershipEnforcements::TenantId,
                            )
                            .from_col(GroupMembershipEnforcements::MembershipId)
                            .to(GroupMemberships::Table, GroupMemberships::TenantId)
                            .to_col(GroupMemberships::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("idx_group_membership_enforcements_tenant_group_state")
                .table(GroupMembershipEnforcements::Table)
                .col(GroupMembershipEnforcements::TenantId)
                .col(GroupMembershipEnforcements::GroupId)
                .col(GroupMembershipEnforcements::State)
                .col(GroupMembershipEnforcements::EffectiveUntil)
                .to_owned(),
            Index::create()
                .name("idx_group_membership_enforcements_tenant_user_state")
                .table(GroupMembershipEnforcements::Table)
                .col(GroupMembershipEnforcements::TenantId)
                .col(GroupMembershipEnforcements::UserId)
                .col(GroupMembershipEnforcements::State)
                .col(GroupMembershipEnforcements::EffectiveUntil)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        install_revision_guards(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        remove_revision_guards(manager).await?;
        manager
            .drop_table(
                Table::drop()
                    .table(GroupMembershipEnforcements::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("ux_group_memberships_tenant_id")
                    .table(GroupMemberships::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(GroupMemberships::Table)
                    .drop_column(GroupMemberships::Revision)
                    .to_owned(),
            )
            .await
    }
}

async fn install_revision_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    match manager.get_database_backend() {
        DatabaseBackend::Postgres => manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE OR REPLACE FUNCTION groups_guard_membership_revision()
RETURNS trigger AS $$
BEGIN
    IF NEW.revision < OLD.revision THEN
        RAISE EXCEPTION 'group membership revision must be monotonic';
    END IF;

    IF (
        NEW.role IS DISTINCT FROM OLD.role OR
        NEW.status IS DISTINCT FROM OLD.status OR
        NEW.invited_by_user_id IS DISTINCT FROM OLD.invited_by_user_id OR
        NEW.joined_at IS DISTINCT FROM OLD.joined_at OR
        NEW.left_at IS DISTINCT FROM OLD.left_at
    ) AND NEW.revision <= OLD.revision THEN
        NEW.revision := OLD.revision + 1;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS groups_20_membership_revision_guard ON group_memberships;
CREATE TRIGGER groups_20_membership_revision_guard
BEFORE UPDATE ON group_memberships
FOR EACH ROW EXECUTE FUNCTION groups_guard_membership_revision();

CREATE OR REPLACE FUNCTION groups_bump_membership_revision_from_enforcement()
RETURNS trigger AS $$
DECLARE
    row_tenant_id uuid;
    row_membership_id uuid;
BEGIN
    IF TG_OP = 'DELETE' THEN
        row_tenant_id := OLD.tenant_id;
        row_membership_id := OLD.membership_id;
    ELSE
        row_tenant_id := NEW.tenant_id;
        row_membership_id := NEW.membership_id;
    END IF;

    UPDATE group_memberships
       SET revision = revision + 1,
           updated_at = CURRENT_TIMESTAMP
     WHERE tenant_id = row_tenant_id
       AND id = row_membership_id;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS groups_30_enforcement_membership_revision ON group_membership_enforcements;
CREATE TRIGGER groups_30_enforcement_membership_revision
AFTER INSERT OR UPDATE OR DELETE ON group_membership_enforcements
FOR EACH ROW EXECUTE FUNCTION groups_bump_membership_revision_from_enforcement();
"#,
            )
            .await
            .map(|_| ()),
        DatabaseBackend::Sqlite => manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TRIGGER groups_10_membership_revision_monotonic
BEFORE UPDATE OF revision ON group_memberships
FOR EACH ROW
WHEN NEW.revision < OLD.revision
BEGIN
    SELECT RAISE(ABORT, 'group membership revision must be monotonic');
END;

CREATE TRIGGER groups_20_membership_revision_bump
AFTER UPDATE OF role, status, invited_by_user_id, joined_at, left_at ON group_memberships
FOR EACH ROW
WHEN NEW.revision <= OLD.revision
BEGIN
    UPDATE group_memberships
       SET revision = OLD.revision + 1
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.id;
END;

CREATE TRIGGER groups_30_enforcement_membership_revision_insert
AFTER INSERT ON group_membership_enforcements
FOR EACH ROW
BEGIN
    UPDATE group_memberships
       SET revision = revision + 1,
           updated_at = CURRENT_TIMESTAMP
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.membership_id;
END;

CREATE TRIGGER groups_31_enforcement_membership_revision_update
AFTER UPDATE ON group_membership_enforcements
FOR EACH ROW
BEGIN
    UPDATE group_memberships
       SET revision = revision + 1,
           updated_at = CURRENT_TIMESTAMP
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.membership_id;
END;

CREATE TRIGGER groups_32_enforcement_membership_revision_delete
AFTER DELETE ON group_membership_enforcements
FOR EACH ROW
BEGIN
    UPDATE group_memberships
       SET revision = revision + 1,
           updated_at = CURRENT_TIMESTAMP
     WHERE tenant_id = OLD.tenant_id
       AND id = OLD.membership_id;
END;
"#,
            )
            .await
            .map(|_| ()),
        backend => Err(DbErr::Custom(format!(
            "groups membership enforcement migration does not support {backend:?}"
        ))),
    }
}

async fn remove_revision_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    match manager.get_database_backend() {
        DatabaseBackend::Postgres => manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS groups_30_enforcement_membership_revision ON group_membership_enforcements;
DROP TRIGGER IF EXISTS groups_20_membership_revision_guard ON group_memberships;
DROP FUNCTION IF EXISTS groups_bump_membership_revision_from_enforcement();
DROP FUNCTION IF EXISTS groups_guard_membership_revision();
"#,
            )
            .await
            .map(|_| ()),
        DatabaseBackend::Sqlite => manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS groups_32_enforcement_membership_revision_delete;
DROP TRIGGER IF EXISTS groups_31_enforcement_membership_revision_update;
DROP TRIGGER IF EXISTS groups_30_enforcement_membership_revision_insert;
DROP TRIGGER IF EXISTS groups_20_membership_revision_bump;
DROP TRIGGER IF EXISTS groups_10_membership_revision_monotonic;
"#,
            )
            .await
            .map(|_| ()),
        backend => Err(DbErr::Custom(format!(
            "groups membership enforcement migration does not support {backend:?}"
        ))),
    }
}

#[derive(DeriveIden)]
enum GroupMemberships {
    Table,
    Id,
    TenantId,
    Revision,
}

#[derive(DeriveIden)]
enum GroupMembershipEnforcements {
    Table,
    MembershipId,
    TenantId,
    GroupId,
    UserId,
    State,
    ReasonCode,
    SourceKind,
    EffectiveFrom,
    EffectiveUntil,
    RestoreStatus,
    ModerationDecisionId,
    ModerationDecisionHash,
    ActorKind,
    ActorId,
    Revision,
    RevokedAt,
    CreatedAt,
    UpdatedAt,
}
