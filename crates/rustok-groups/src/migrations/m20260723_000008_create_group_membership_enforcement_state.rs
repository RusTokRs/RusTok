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

CREATE OR REPLACE FUNCTION groups_guard_membership_enforcement()
RETURNS trigger AS $$
DECLARE
    identity_exists boolean;
BEGIN
    SELECT EXISTS (
        SELECT 1
          FROM group_memberships membership
         WHERE membership.tenant_id = NEW.tenant_id
           AND membership.id = NEW.membership_id
           AND membership.group_id = NEW.group_id
           AND membership.user_id = NEW.user_id
    ) INTO identity_exists;

    IF NOT identity_exists THEN
        RAISE EXCEPTION 'group membership enforcement identity does not match membership';
    END IF;

    IF TG_OP = 'UPDATE' THEN
        IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
           OR NEW.membership_id IS DISTINCT FROM OLD.membership_id
           OR NEW.group_id IS DISTINCT FROM OLD.group_id
           OR NEW.user_id IS DISTINCT FROM OLD.user_id THEN
            RAISE EXCEPTION 'group membership enforcement identity is immutable';
        END IF;
        IF NEW.revision < OLD.revision THEN
            RAISE EXCEPTION 'group membership enforcement revision must be monotonic';
        END IF;
        IF (
            NEW.state IS DISTINCT FROM OLD.state OR
            NEW.reason_code IS DISTINCT FROM OLD.reason_code OR
            NEW.source_kind IS DISTINCT FROM OLD.source_kind OR
            NEW.effective_from IS DISTINCT FROM OLD.effective_from OR
            NEW.effective_until IS DISTINCT FROM OLD.effective_until OR
            NEW.restore_status IS DISTINCT FROM OLD.restore_status OR
            NEW.moderation_decision_id IS DISTINCT FROM OLD.moderation_decision_id OR
            NEW.moderation_decision_hash IS DISTINCT FROM OLD.moderation_decision_hash OR
            NEW.actor_kind IS DISTINCT FROM OLD.actor_kind OR
            NEW.actor_id IS DISTINCT FROM OLD.actor_id OR
            NEW.revoked_at IS DISTINCT FROM OLD.revoked_at
        ) AND NEW.revision <= OLD.revision THEN
            NEW.revision := OLD.revision + 1;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS groups_25_membership_enforcement_guard ON group_membership_enforcements;
CREATE TRIGGER groups_25_membership_enforcement_guard
BEFORE INSERT OR UPDATE ON group_membership_enforcements
FOR EACH ROW EXECUTE FUNCTION groups_guard_membership_enforcement();

CREATE OR REPLACE FUNCTION groups_bump_membership_revision_from_enforcement()
RETURNS trigger AS $$
DECLARE
    row_tenant_id uuid;
    row_membership_id uuid;
    material_change boolean := true;
BEGIN
    IF TG_OP = 'DELETE' THEN
        row_tenant_id := OLD.tenant_id;
        row_membership_id := OLD.membership_id;
    ELSE
        row_tenant_id := NEW.tenant_id;
        row_membership_id := NEW.membership_id;
    END IF;

    IF TG_OP = 'UPDATE' THEN
        material_change :=
            NEW.state IS DISTINCT FROM OLD.state OR
            NEW.reason_code IS DISTINCT FROM OLD.reason_code OR
            NEW.source_kind IS DISTINCT FROM OLD.source_kind OR
            NEW.effective_from IS DISTINCT FROM OLD.effective_from OR
            NEW.effective_until IS DISTINCT FROM OLD.effective_until OR
            NEW.restore_status IS DISTINCT FROM OLD.restore_status OR
            NEW.moderation_decision_id IS DISTINCT FROM OLD.moderation_decision_id OR
            NEW.moderation_decision_hash IS DISTINCT FROM OLD.moderation_decision_hash OR
            NEW.actor_kind IS DISTINCT FROM OLD.actor_kind OR
            NEW.actor_id IS DISTINCT FROM OLD.actor_id OR
            NEW.revoked_at IS DISTINCT FROM OLD.revoked_at;
    END IF;

    IF material_change THEN
        UPDATE group_memberships
           SET revision = revision + 1,
               updated_at = CURRENT_TIMESTAMP
         WHERE tenant_id = row_tenant_id
           AND id = row_membership_id;
    END IF;

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

CREATE TRIGGER groups_24_membership_enforcement_identity_insert
BEFORE INSERT ON group_membership_enforcements
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
      FROM group_memberships membership
     WHERE membership.tenant_id = NEW.tenant_id
       AND membership.id = NEW.membership_id
       AND membership.group_id = NEW.group_id
       AND membership.user_id = NEW.user_id
)
BEGIN
    SELECT RAISE(ABORT, 'group membership enforcement identity does not match membership');
END;

CREATE TRIGGER groups_25_membership_enforcement_identity_update
BEFORE UPDATE ON group_membership_enforcements
FOR EACH ROW
WHEN NEW.tenant_id IS NOT OLD.tenant_id
   OR NEW.membership_id IS NOT OLD.membership_id
   OR NEW.group_id IS NOT OLD.group_id
   OR NEW.user_id IS NOT OLD.user_id
   OR NOT EXISTS (
       SELECT 1
         FROM group_memberships membership
        WHERE membership.tenant_id = NEW.tenant_id
          AND membership.id = NEW.membership_id
          AND membership.group_id = NEW.group_id
          AND membership.user_id = NEW.user_id
   )
BEGIN
    SELECT RAISE(ABORT, 'group membership enforcement identity is immutable or invalid');
END;

CREATE TRIGGER groups_26_membership_enforcement_revision_monotonic
BEFORE UPDATE OF revision ON group_membership_enforcements
FOR EACH ROW
WHEN NEW.revision < OLD.revision
BEGIN
    SELECT RAISE(ABORT, 'group membership enforcement revision must be monotonic');
END;

CREATE TRIGGER groups_27_membership_enforcement_revision_bump
AFTER UPDATE OF state, reason_code, source_kind, effective_from, effective_until,
    restore_status, moderation_decision_id, moderation_decision_hash, actor_kind, actor_id,
    revoked_at ON group_membership_enforcements
FOR EACH ROW
WHEN NEW.revision <= OLD.revision
 AND (
    NEW.state IS NOT OLD.state OR
    NEW.reason_code IS NOT OLD.reason_code OR
    NEW.source_kind IS NOT OLD.source_kind OR
    NEW.effective_from IS NOT OLD.effective_from OR
    NEW.effective_until IS NOT OLD.effective_until OR
    NEW.restore_status IS NOT OLD.restore_status OR
    NEW.moderation_decision_id IS NOT OLD.moderation_decision_id OR
    NEW.moderation_decision_hash IS NOT OLD.moderation_decision_hash OR
    NEW.actor_kind IS NOT OLD.actor_kind OR
    NEW.actor_id IS NOT OLD.actor_id OR
    NEW.revoked_at IS NOT OLD.revoked_at
 )
BEGIN
    UPDATE group_membership_enforcements
       SET revision = OLD.revision + 1
     WHERE tenant_id = NEW.tenant_id
       AND membership_id = NEW.membership_id;
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
AFTER UPDATE OF state, reason_code, source_kind, effective_from, effective_until,
    restore_status, moderation_decision_id, moderation_decision_hash, actor_kind, actor_id,
    revoked_at ON group_membership_enforcements
FOR EACH ROW
WHEN NEW.state IS NOT OLD.state
  OR NEW.reason_code IS NOT OLD.reason_code
  OR NEW.source_kind IS NOT OLD.source_kind
  OR NEW.effective_from IS NOT OLD.effective_from
  OR NEW.effective_until IS NOT OLD.effective_until
  OR NEW.restore_status IS NOT OLD.restore_status
  OR NEW.moderation_decision_id IS NOT OLD.moderation_decision_id
  OR NEW.moderation_decision_hash IS NOT OLD.moderation_decision_hash
  OR NEW.actor_kind IS NOT OLD.actor_kind
  OR NEW.actor_id IS NOT OLD.actor_id
  OR NEW.revoked_at IS NOT OLD.revoked_at
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
DROP TRIGGER IF EXISTS groups_25_membership_enforcement_guard ON group_membership_enforcements;
DROP TRIGGER IF EXISTS groups_20_membership_revision_guard ON group_memberships;
DROP FUNCTION IF EXISTS groups_bump_membership_revision_from_enforcement();
DROP FUNCTION IF EXISTS groups_guard_membership_enforcement();
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
DROP TRIGGER IF EXISTS groups_27_membership_enforcement_revision_bump;
DROP TRIGGER IF EXISTS groups_26_membership_enforcement_revision_monotonic;
DROP TRIGGER IF EXISTS groups_25_membership_enforcement_identity_update;
DROP TRIGGER IF EXISTS groups_24_membership_enforcement_identity_insert;
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
