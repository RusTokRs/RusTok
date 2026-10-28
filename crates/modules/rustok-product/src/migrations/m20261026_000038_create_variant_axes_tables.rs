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
        // Portable counterpart of the table DDL in
        // m20260918_000033_create_variant_axes_and_invariants. That migration
        // is PostgreSQL-only because its invariant triggers are PL/pgSQL, so
        // on SQLite the catalog service had no variant-axis tables at all and
        // every product write failed with "no such table:
        // product_variant_axes". PostgreSQL already received these tables
        // from 000033; IF NOT EXISTS keeps this migration a no-op there.
        // Foreign keys here are single-column identity keys (the portable
        // pattern used by m20261018_000037); the composite tenant-scoped
        // guards remain PostgreSQL-side, as with the other product
        // invariant migrations that SQLite skips.
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("product_variant_axes"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("product_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("attribute_id")).uuid().not_null())
                    .col(
                        ColumnDef::new(Alias::new("position"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_variant_axes_product")
                            .from(Alias::new("product_variant_axes"), Alias::new("product_id"))
                            .to(Alias::new("products"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_variant_axes_attribute")
                            .from(
                                Alias::new("product_variant_axes"),
                                Alias::new("attribute_id"),
                            )
                            .to(Alias::new("product_attributes"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("product_variant_axis_values"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("axis_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("option_id")).uuid().not_null())
                    .col(
                        ColumnDef::new(Alias::new("position"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_variant_axis_values_axis")
                            .from(
                                Alias::new("product_variant_axis_values"),
                                Alias::new("axis_id"),
                            )
                            .to(Alias::new("product_variant_axes"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_variant_axis_values_option")
                            .from(
                                Alias::new("product_variant_axis_values"),
                                Alias::new("option_id"),
                            )
                            .to(Alias::new("product_attribute_options"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("product_attribute_groups"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(
                        ColumnDef::new(Alias::new("code"))
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("position"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_attribute_groups_tenant")
                            .from(
                                Alias::new("product_attribute_groups"),
                                Alias::new("tenant_id"),
                            )
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("product_attribute_group_translations"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("group_id")).uuid().not_null())
                    .col(
                        ColumnDef::new(Alias::new("locale"))
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("label"))
                            .string_len(255)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_attribute_group_translations_tenant")
                            .from(
                                Alias::new("product_attribute_group_translations"),
                                Alias::new("tenant_id"),
                            )
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_attribute_group_translations_group")
                            .from(
                                Alias::new("product_attribute_group_translations"),
                                Alias::new("group_id"),
                            )
                            .to(Alias::new("product_attribute_groups"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("product_attribute_group_attributes"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("tenant_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("group_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("attribute_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("position"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_attribute_group_attributes_tenant")
                            .from(
                                Alias::new("product_attribute_group_attributes"),
                                Alias::new("tenant_id"),
                            )
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_attribute_group_attributes_group")
                            .from(
                                Alias::new("product_attribute_group_attributes"),
                                Alias::new("group_id"),
                            )
                            .to(Alias::new("product_attribute_groups"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_attribute_group_attributes_attribute")
                            .from(
                                Alias::new("product_attribute_group_attributes"),
                                Alias::new("attribute_id"),
                            )
                            .to(Alias::new("product_attributes"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("product_attribute_group_channel_settings"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("group_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("channel_id")).uuid().not_null())
                    .col(
                        ColumnDef::new(Alias::new("is_visible"))
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(Alias::new("position")).integer())
                    .col(
                        ColumnDef::new(Alias::new("metadata"))
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_attribute_group_channel_settings_tenant")
                            .from(
                                Alias::new("product_attribute_group_channel_settings"),
                                Alias::new("tenant_id"),
                            )
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_attribute_group_channel_settings_group")
                            .from(
                                Alias::new("product_attribute_group_channel_settings"),
                                Alias::new("group_id"),
                            )
                            .to(Alias::new("product_attribute_groups"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            // Mirror the named UNIQUE constraints of the PostgreSQL DDL.
            // PostgreSQL already enforces them as table constraints from
            // 000033, so the indexes are only created for SQLite.
            for (name, table, columns) in [
                (
                    "uq_product_variant_axes_product_attribute",
                    "product_variant_axes",
                    ["tenant_id", "product_id", "attribute_id"].as_slice(),
                ),
                (
                    "uq_product_variant_axes_product_position",
                    "product_variant_axes",
                    ["tenant_id", "product_id", "position"].as_slice(),
                ),
                (
                    "uq_product_variant_axis_values_axis_option",
                    "product_variant_axis_values",
                    ["tenant_id", "axis_id", "option_id"].as_slice(),
                ),
                (
                    "uq_product_variant_axis_values_axis_position",
                    "product_variant_axis_values",
                    ["tenant_id", "axis_id", "position"].as_slice(),
                ),
                (
                    "uq_product_attribute_groups_tenant_code",
                    "product_attribute_groups",
                    ["tenant_id", "code"].as_slice(),
                ),
                (
                    "uq_product_attribute_group_translations",
                    "product_attribute_group_translations",
                    ["tenant_id", "group_id", "locale"].as_slice(),
                ),
                (
                    "uq_product_attribute_group_channel_settings",
                    "product_attribute_group_channel_settings",
                    ["tenant_id", "group_id", "channel_id"].as_slice(),
                ),
            ] {
                let mut index = Index::create()
                    .name(name)
                    .table(Alias::new(table))
                    .unique()
                    .to_owned();
                for column in columns {
                    index.col(Alias::new(*column));
                }
                manager.create_index(index).await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // The tables are owned by m20260918_000033 on PostgreSQL; this
        // migration only created them on SQLite.
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            return Ok(());
        }

        for table in [
            "product_attribute_group_channel_settings",
            "product_attribute_group_attributes",
            "product_attribute_group_translations",
            "product_attribute_groups",
            "product_variant_axis_values",
            "product_variant_axes",
        ] {
            manager
                .drop_table(
                    Table::drop()
                        .table(Alias::new(table))
                        .if_exists()
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}
