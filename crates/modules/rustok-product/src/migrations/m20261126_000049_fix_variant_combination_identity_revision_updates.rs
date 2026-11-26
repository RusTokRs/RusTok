use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

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
CREATE OR REPLACE FUNCTION rustok_product_maintain_combination_identity()
RETURNS TRIGGER AS $$
DECLARE
    v_variant_id UUID;
    v_tenant_id UUID;
    v_product_id UUID;
    computed_identity TEXT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        SELECT pva.variant_id, pva.tenant_id INTO v_variant_id, v_tenant_id
        FROM product_variant_attribute_values pva
        WHERE pva.id = OLD.value_id;
    ELSE
        SELECT pva.variant_id, pva.tenant_id INTO v_variant_id, v_tenant_id
        FROM product_variant_attribute_values pva
        WHERE pva.id = NEW.value_id;
    END IF;

    IF v_variant_id IS NOT NULL THEN
        SELECT pv.product_id INTO v_product_id
        FROM product_variants pv
        WHERE pv.tenant_id = v_tenant_id AND pv.id = v_variant_id;

        IF v_product_id IS NOT NULL THEN
            computed_identity := rustok_product_compute_combination_identity(
                v_variant_id, v_product_id, v_tenant_id
            );
            -- Product writes may store this derived identity before COMMIT. Deferred row
            -- triggers must not rewrite the same Variant repeatedly: each UPDATE advances
            -- its business revision and Index watermark.
            UPDATE product_variants
            SET combination_identity = computed_identity
            WHERE tenant_id = v_tenant_id
              AND id = v_variant_id
              AND combination_identity IS DISTINCT FROM computed_identity;
        END IF;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;
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
CREATE OR REPLACE FUNCTION rustok_product_maintain_combination_identity()
RETURNS TRIGGER AS $$
DECLARE
    v_variant_id UUID;
    v_tenant_id UUID;
    v_product_id UUID;
BEGIN
    IF TG_OP = 'DELETE' THEN
        SELECT pva.variant_id, pva.tenant_id INTO v_variant_id, v_tenant_id
        FROM product_variant_attribute_values pva
        WHERE pva.id = OLD.value_id;
    ELSE
        SELECT pva.variant_id, pva.tenant_id INTO v_variant_id, v_tenant_id
        FROM product_variant_attribute_values pva
        WHERE pva.id = NEW.value_id;
    END IF;

    IF v_variant_id IS NOT NULL THEN
        SELECT pv.product_id INTO v_product_id
        FROM product_variants pv
        WHERE pv.tenant_id = v_tenant_id AND pv.id = v_variant_id;

        IF v_product_id IS NOT NULL THEN
            UPDATE product_variants
            SET combination_identity = rustok_product_compute_combination_identity(
                v_variant_id, v_product_id, v_tenant_id
            )
            WHERE tenant_id = v_tenant_id AND id = v_variant_id;
        END IF;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;
"#,
            )
            .await?;

        Ok(())
    }
}
