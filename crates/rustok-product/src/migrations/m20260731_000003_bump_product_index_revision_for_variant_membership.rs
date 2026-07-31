use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE OR REPLACE FUNCTION rustok_product_variant_membership_bump_index_revision()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE tenant_id = NEW.tenant_id
          AND id = NEW.product_id;
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE tenant_id = OLD.tenant_id
          AND id = OLD.product_id;
        RETURN OLD;
    END IF;

    IF OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.product_id IS NOT DISTINCT FROM NEW.product_id
       AND OLD.id IS NOT DISTINCT FROM NEW.id THEN
        RETURN NEW;
    END IF;

    UPDATE products
    SET index_revision = index_revision + 1
    WHERE tenant_id = OLD.tenant_id
      AND id = OLD.product_id;

    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
       OR OLD.product_id IS DISTINCT FROM NEW.product_id THEN
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE tenant_id = NEW.tenant_id
          AND id = NEW.product_id;
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_variants_membership_insert_revision
AFTER INSERT ON product_variants
FOR EACH ROW
EXECUTE FUNCTION rustok_product_variant_membership_bump_index_revision();

CREATE TRIGGER trg_product_variants_membership_delete_revision
AFTER DELETE ON product_variants
FOR EACH ROW
EXECUTE FUNCTION rustok_product_variant_membership_bump_index_revision();

CREATE TRIGGER trg_product_variants_membership_update_revision
AFTER UPDATE OF id, tenant_id, product_id ON product_variants
FOR EACH ROW
EXECUTE FUNCTION rustok_product_variant_membership_bump_index_revision();
"#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS trg_product_variants_membership_update_revision ON product_variants;
DROP TRIGGER IF EXISTS trg_product_variants_membership_delete_revision ON product_variants;
DROP TRIGGER IF EXISTS trg_product_variants_membership_insert_revision ON product_variants;
DROP FUNCTION IF EXISTS rustok_product_variant_membership_bump_index_revision();
"#,
            )
            .await?;
        Ok(())
    }
}
