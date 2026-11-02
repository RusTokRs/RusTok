use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Create carts table
        manager
            .create_table(
                Table::create()
                    .table(Carts::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Carts::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Carts::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Carts::ChannelId).uuid().null())
                    .col(ColumnDef::new(Carts::ChannelSlug).string_len(64).null())
                    .col(ColumnDef::new(Carts::CustomerId).uuid().null())
                    .col(ColumnDef::new(Carts::Email).string_len(255).null())
                    .col(ColumnDef::new(Carts::RegionId).uuid().null())
                    .col(ColumnDef::new(Carts::CountryCode).string_len(2).null())
                    .col(ColumnDef::new(Carts::LocaleCode).string_len(32).null())
                    .col(ColumnDef::new(Carts::SelectedShippingOptionId).uuid().null())
                    .col(
                        ColumnDef::new(Carts::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(ColumnDef::new(Carts::CurrencyCode).string_len(3).not_null())
                    .col(
                        ColumnDef::new(Carts::ShippingTotal)
                            .decimal_len(12, 4)
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Carts::TotalAmount)
                            .decimal_len(12, 4)
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Carts::TaxTotal)
                            .decimal_len(12, 4)
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Carts::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Carts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Carts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Carts::CompletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_carts_tenant_customer")
                    .table(Carts::Table)
                    .col(Carts::TenantId)
                    .col(Carts::CustomerId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_carts_tenant_status")
                    .table(Carts::Table)
                    .col(Carts::TenantId)
                    .col(Carts::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_carts_tenant_channel")
                    .table(Carts::Table)
                    .col(Carts::TenantId)
                    .col(Carts::ChannelSlug)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_carts_selected_shipping_option")
                    .table(Carts::Table)
                    .col(Carts::SelectedShippingOptionId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 2. Create cart_line_items table
        manager
            .create_table(
                Table::create()
                    .table(CartLineItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartLineItems::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CartLineItems::CartId).uuid().not_null())
                    .col(ColumnDef::new(CartLineItems::ProductId).uuid().null())
                    .col(ColumnDef::new(CartLineItems::VariantId).uuid().null())
                    .col(
                        ColumnDef::new(CartLineItems::FulfillmentRequirement)
                            .string_len(16)
                            .not_null()
                            .default("physical"),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::ShippingProfileSlug)
                            .string_len(64)
                            .not_null()
                            .default(""),
                    )
                    .col(ColumnDef::new(CartLineItems::Sku).string_len(64).null())
                    .col(ColumnDef::new(CartLineItems::Quantity).integer().not_null())
                    .col(
                        ColumnDef::new(CartLineItems::UnitPrice)
                            .decimal_len(12, 4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::TotalPrice)
                            .decimal_len(12, 4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_line_items_cart")
                            .from(CartLineItems::Table, CartLineItems::CartId)
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_line_items_cart")
                    .table(CartLineItems::Table)
                    .col(CartLineItems::CartId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_line_items_variant")
                    .table(CartLineItems::Table)
                    .col(CartLineItems::VariantId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 3. Create cart_adjustments table
        manager
            .create_table(
                Table::create()
                    .table(CartAdjustments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartAdjustments::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CartAdjustments::CartId).uuid().not_null())
                    .col(ColumnDef::new(CartAdjustments::CartLineItemId).uuid().null())
                    .col(
                        ColumnDef::new(CartAdjustments::SourceType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartAdjustments::SourceId)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(CartAdjustments::Amount)
                            .decimal_len(12, 4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartAdjustments::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartAdjustments::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(CartAdjustments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartAdjustments::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_adjustments_cart")
                            .from(CartAdjustments::Table, CartAdjustments::CartId)
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_adjustments_line_item")
                            .from(CartAdjustments::Table, CartAdjustments::CartLineItemId)
                            .to(CartLineItems::Table, CartLineItems::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_adjustments_cart")
                    .table(CartAdjustments::Table)
                    .col(CartAdjustments::CartId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_adjustments_line_item")
                    .table(CartAdjustments::Table)
                    .col(CartAdjustments::CartLineItemId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 4. Create cart_tax_lines table
        manager
            .create_table(
                Table::create()
                    .table(CartTaxLines::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartTaxLines::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CartTaxLines::CartId).uuid().not_null())
                    .col(ColumnDef::new(CartTaxLines::CartLineItemId).uuid().null())
                    .col(ColumnDef::new(CartTaxLines::ShippingOptionId).uuid().null())
                    .col(ColumnDef::new(CartTaxLines::Description).string_len(255).null())
                    .col(
                        ColumnDef::new(CartTaxLines::ProviderId)
                            .string_len(64)
                            .not_null()
                            .default("system"),
                    )
                    .col(
                        ColumnDef::new(CartTaxLines::Rate)
                            .decimal_len(6, 4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartTaxLines::Amount)
                            .decimal_len(12, 4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartTaxLines::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartTaxLines::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(CartTaxLines::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartTaxLines::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_tax_lines_cart")
                            .from(CartTaxLines::Table, CartTaxLines::CartId)
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_tax_lines_line_item")
                            .from(CartTaxLines::Table, CartTaxLines::CartLineItemId)
                            .to(CartLineItems::Table, CartLineItems::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_tax_lines_cart")
                    .table(CartTaxLines::Table)
                    .col(CartTaxLines::CartId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_tax_lines_line_item")
                    .table(CartTaxLines::Table)
                    .col(CartTaxLines::CartLineItemId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 5. Create cart_line_item_translations table
        manager
            .create_table(
                Table::create()
                    .table(CartLineItemTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartLineItemTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemTranslations::CartLineItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemTranslations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemTranslations::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemTranslations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartLineItemTranslations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_line_item_translations_item")
                            .from(
                                CartLineItemTranslations::Table,
                                CartLineItemTranslations::CartLineItemId,
                            )
                            .to(CartLineItems::Table, CartLineItems::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_line_item_translations_item_locale")
                    .table(CartLineItemTranslations::Table)
                    .col(CartLineItemTranslations::CartLineItemId)
                    .col(CartLineItemTranslations::Locale)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 6. Create cart_shipping_selections table
        manager
            .create_table(
                Table::create()
                    .table(CartShippingSelections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartShippingSelections::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CartShippingSelections::CartId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartShippingSelections::ShippingProfileSlug)
                            .string_len(64)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(CartShippingSelections::SellerId)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(CartShippingSelections::SellerScope)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(CartShippingSelections::SelectedShippingOptionId)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(CartShippingSelections::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartShippingSelections::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_shipping_selections_cart")
                            .from(
                                CartShippingSelections::Table,
                                CartShippingSelections::CartId,
                            )
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_shipping_selections_cart")
                    .table(CartShippingSelections::Table)
                    .col(CartShippingSelections::CartId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_shipping_selections_option")
                    .table(CartShippingSelections::Table)
                    .col(CartShippingSelections::SelectedShippingOptionId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 7. Create cart_line_item_marketplace_snapshots table
        manager
            .create_table(
                Table::create()
                    .table(CartLineItemMarketplaceSnapshots::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::CartLineItemId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::SellerId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::ListingId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::MasterProductId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::MasterVariantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::ListingTermsVersion)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::CurrencyExponent)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::UnitAmount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::SubtotalAmount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::DiscountAmount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::TaxAmount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::TotalAmount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::PricingReference)
                            .string_len(255)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::InventoryReference)
                            .string_len(255)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::FulfillmentProfileSlug)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartLineItemMarketplaceSnapshots::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_line_item_marketplace_snapshots_item")
                            .from(
                                CartLineItemMarketplaceSnapshots::Table,
                                CartLineItemMarketplaceSnapshots::CartLineItemId,
                            )
                            .to(CartLineItems::Table, CartLineItems::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 8. Database-specific constraints and triggers
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                install_postgres_constraints(manager).await?;
            }
            DatabaseBackend::Sqlite => {
                install_sqlite_constraints(manager).await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CartLineItemMarketplaceSnapshots::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CartShippingSelections::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CartLineItemTranslations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CartTaxLines::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CartAdjustments::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CartLineItems::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Carts::Table).if_exists().to_owned())
            .await?;
        Ok(())
    }
}

async fn install_postgres_constraints(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE carts
                ADD CONSTRAINT ck_carts_status
                CHECK (status IN ('active', 'checking_out', 'completed', 'abandoned')),
                ADD CONSTRAINT ck_carts_currency
                CHECK (currency_code ~ '^[A-Z]{3}$'),
                ADD CONSTRAINT ck_carts_money
                CHECK (total_amount >= 0 AND shipping_total >= 0 AND tax_total >= 0),
                ADD CONSTRAINT ck_carts_completion_state
                CHECK (
                    (status = 'completed' AND completed_at IS NOT NULL)
                    OR (status <> 'completed' AND completed_at IS NULL)
                );

            ALTER TABLE cart_line_items
                ADD CONSTRAINT ck_cart_line_items_quantity
                CHECK (quantity > 0),
                ADD CONSTRAINT ck_cart_line_items_currency
                CHECK (currency_code ~ '^[A-Z]{3}$'),
                ADD CONSTRAINT ck_cart_line_items_money
                CHECK (
                    unit_price >= 0
                    AND total_price >= 0
                    AND total_price = unit_price * quantity
                ),
                ADD CONSTRAINT ck_cart_line_items_fulfillment_requirement
                CHECK (fulfillment_requirement IN ('digital', 'physical')),
                ADD CONSTRAINT ck_cart_line_items_fulfillment_shipping
                CHECK (
                    (fulfillment_requirement = 'digital' AND shipping_profile_slug = '')
                    OR (fulfillment_requirement = 'physical' AND btrim(shipping_profile_slug) <> '')
                );

            ALTER TABLE cart_adjustments
                ADD CONSTRAINT ck_cart_adjustments_amount
                CHECK (amount > 0),
                ADD CONSTRAINT ck_cart_adjustments_currency
                CHECK (currency_code ~ '^[A-Z]{3}$'),
                ADD CONSTRAINT ck_cart_adjustments_source
                CHECK (btrim(source_type) <> '');

            ALTER TABLE cart_tax_lines
                ADD CONSTRAINT ck_cart_tax_lines_amounts
                CHECK (rate >= 0 AND amount >= 0),
                ADD CONSTRAINT ck_cart_tax_lines_currency
                CHECK (currency_code ~ '^[A-Z]{3}$'),
                ADD CONSTRAINT ck_cart_tax_lines_provider
                CHECK (btrim(provider_id) <> ''),
                ADD CONSTRAINT ck_cart_tax_lines_target
                CHECK ((cart_line_item_id IS NULL) <> (shipping_option_id IS NULL));

            CREATE OR REPLACE FUNCTION enforce_cart_lifecycle_transition() RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id THEN
                    RAISE EXCEPTION 'cart identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.status = OLD.status THEN
                    RAISE EXCEPTION 'stale cart lifecycle update for status %', OLD.status
                        USING ERRCODE = '40001';
                END IF;

                IF NOT (
                    (OLD.status = 'active' AND NEW.status IN ('checking_out', 'completed', 'abandoned'))
                    OR (OLD.status = 'checking_out' AND NEW.status IN ('active', 'completed'))
                ) THEN
                    RAISE EXCEPTION 'invalid cart transition from % to %', OLD.status, NEW.status
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER carts_lifecycle_transition_guard
            BEFORE UPDATE OF status ON carts
            FOR EACH ROW
            EXECUTE FUNCTION enforce_cart_lifecycle_transition();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite_constraints(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER carts_integrity_guard_insert
            BEFORE INSERT ON carts FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.status NOT IN ('active', 'checking_out', 'completed', 'abandoned')
                    THEN RAISE(ABORT, 'invalid cart status') END;
                SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                    THEN RAISE(ABORT, 'invalid cart currency') END;
                SELECT CASE WHEN NEW.total_amount < 0 OR NEW.shipping_total < 0 OR NEW.tax_total < 0
                    THEN RAISE(ABORT, 'invalid cart monetary totals') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'completed' AND NEW.completed_at IS NOT NULL)
                    OR (NEW.status <> 'completed' AND NEW.completed_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid cart completion state') END;
            END;

            CREATE TRIGGER carts_integrity_guard_update
            BEFORE UPDATE OF status, currency_code, total_amount, shipping_total, tax_total, completed_at
            ON carts FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.status NOT IN ('active', 'checking_out', 'completed', 'abandoned')
                    THEN RAISE(ABORT, 'invalid cart status') END;
                SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                    THEN RAISE(ABORT, 'invalid cart currency') END;
                SELECT CASE WHEN NEW.total_amount < 0 OR NEW.shipping_total < 0 OR NEW.tax_total < 0
                    THEN RAISE(ABORT, 'invalid cart monetary totals') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'completed' AND NEW.completed_at IS NOT NULL)
                    OR (NEW.status <> 'completed' AND NEW.completed_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid cart completion state') END;
            END;

            CREATE TRIGGER carts_lifecycle_transition_guard
            BEFORE UPDATE OF status ON carts
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id OR NEW.tenant_id IS NOT OLD.tenant_id
                    THEN RAISE(ABORT, 'cart identity is immutable') END;
                SELECT CASE WHEN NEW.status = OLD.status
                    THEN RAISE(ABORT, 'stale cart lifecycle update') END;
                SELECT CASE WHEN NOT (
                    (OLD.status = 'active' AND NEW.status IN ('checking_out', 'completed', 'abandoned'))
                    OR (OLD.status = 'checking_out' AND NEW.status IN ('active', 'completed'))
                ) THEN RAISE(ABORT, 'invalid cart transition') END;
            END;

            CREATE TRIGGER cart_line_items_integrity_guard_insert
            BEFORE INSERT ON cart_line_items
            FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.quantity <= 0 OR NEW.unit_price < 0 OR NEW.total_price < 0
                    OR NEW.total_price <> NEW.unit_price * NEW.quantity
                    THEN RAISE(ABORT, 'invalid cart line item money') END;
                SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                    THEN RAISE(ABORT, 'invalid cart line item currency') END;
                SELECT CASE WHEN NEW.fulfillment_requirement NOT IN ('digital', 'physical')
                    THEN RAISE(ABORT, 'invalid fulfillment requirement') END;
                SELECT CASE WHEN (NEW.fulfillment_requirement = 'digital' AND NEW.shipping_profile_slug <> '')
                    OR (NEW.fulfillment_requirement = 'physical' AND trim(NEW.shipping_profile_slug) = '')
                    THEN RAISE(ABORT, 'invalid fulfillment profile slug') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM carts c
                    WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                ) THEN RAISE(ABORT, 'cart line item does not match cart') END;
            END;

            CREATE TRIGGER cart_line_items_integrity_guard_update
            BEFORE UPDATE OF cart_id, quantity, unit_price, total_price, currency_code, fulfillment_requirement, shipping_profile_slug
            ON cart_line_items FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.quantity <= 0 OR NEW.unit_price < 0 OR NEW.total_price < 0
                    OR NEW.total_price <> NEW.unit_price * NEW.quantity
                    THEN RAISE(ABORT, 'invalid cart line item money') END;
                SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                    THEN RAISE(ABORT, 'invalid cart line item currency') END;
                SELECT CASE WHEN NEW.fulfillment_requirement NOT IN ('digital', 'physical')
                    THEN RAISE(ABORT, 'invalid fulfillment requirement') END;
                SELECT CASE WHEN (NEW.fulfillment_requirement = 'digital' AND NEW.shipping_profile_slug <> '')
                    OR (NEW.fulfillment_requirement = 'physical' AND trim(NEW.shipping_profile_slug) = '')
                    THEN RAISE(ABORT, 'invalid fulfillment profile slug') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM carts c
                    WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                ) THEN RAISE(ABORT, 'cart line item does not match cart') END;
            END;

            CREATE TRIGGER cart_adjustments_integrity_guard_insert
            BEFORE INSERT ON cart_adjustments
            FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.amount <= 0 OR trim(NEW.source_type) = ''
                    THEN RAISE(ABORT, 'invalid cart adjustment') END;
                SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                    THEN RAISE(ABORT, 'invalid cart adjustment currency') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM carts c
                    WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                ) THEN RAISE(ABORT, 'cart adjustment does not match cart') END;
                SELECT CASE WHEN NEW.cart_line_item_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM cart_line_items li
                    WHERE li.id = NEW.cart_line_item_id AND li.cart_id = NEW.cart_id
                ) THEN RAISE(ABORT, 'adjustment line item does not belong to cart') END;
            END;

            CREATE TRIGGER cart_adjustments_integrity_guard_update
            BEFORE UPDATE OF cart_id, cart_line_item_id, source_type, amount, currency_code
            ON cart_adjustments FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.amount <= 0 OR trim(NEW.source_type) = ''
                    THEN RAISE(ABORT, 'invalid cart adjustment') END;
                SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                    THEN RAISE(ABORT, 'invalid cart adjustment currency') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM carts c
                    WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                ) THEN RAISE(ABORT, 'cart adjustment does not match cart') END;
                SELECT CASE WHEN NEW.cart_line_item_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM cart_line_items li
                    WHERE li.id = NEW.cart_line_item_id AND li.cart_id = NEW.cart_id
                ) THEN RAISE(ABORT, 'adjustment line item does not belong to cart') END;
            END;

            CREATE TRIGGER cart_tax_lines_integrity_guard_insert
            BEFORE INSERT ON cart_tax_lines
            FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.rate < 0 OR NEW.amount < 0 OR trim(NEW.provider_id) = ''
                    THEN RAISE(ABORT, 'invalid cart tax line') END;
                SELECT CASE WHEN (NEW.cart_line_item_id IS NULL) = (NEW.shipping_option_id IS NULL)
                    THEN RAISE(ABORT, 'tax line requires exactly one taxable target') END;
                SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                    THEN RAISE(ABORT, 'invalid cart tax line currency') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM carts c
                    WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                ) THEN RAISE(ABORT, 'cart tax line does not match cart') END;
                SELECT CASE WHEN NEW.cart_line_item_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM cart_line_items li
                    WHERE li.id = NEW.cart_line_item_id AND li.cart_id = NEW.cart_id
                ) THEN RAISE(ABORT, 'tax line item does not belong to cart') END;
            END;

            CREATE TRIGGER cart_tax_lines_integrity_guard_update
            BEFORE UPDATE OF cart_id, cart_line_item_id, shipping_option_id, rate, amount, currency_code, provider_id
            ON cart_tax_lines FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.rate < 0 OR NEW.amount < 0 OR trim(NEW.provider_id) = ''
                    THEN RAISE(ABORT, 'invalid cart tax line') END;
                SELECT CASE WHEN (NEW.cart_line_item_id IS NULL) = (NEW.shipping_option_id IS NULL)
                    THEN RAISE(ABORT, 'tax line requires exactly one taxable target') END;
                SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                    THEN RAISE(ABORT, 'invalid cart tax line currency') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM carts c
                    WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                ) THEN RAISE(ABORT, 'cart tax line does not match cart') END;
                SELECT CASE WHEN NEW.cart_line_item_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM cart_line_items li
                    WHERE li.id = NEW.cart_line_item_id AND li.cart_id = NEW.cart_id
                ) THEN RAISE(ABORT, 'tax line item does not belong to cart') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(Iden)]
enum Carts {
    Table,
    Id,
    TenantId,
    ChannelId,
    ChannelSlug,
    CustomerId,
    Email,
    RegionId,
    CountryCode,
    LocaleCode,
    SelectedShippingOptionId,
    Status,
    CurrencyCode,
    ShippingTotal,
    TotalAmount,
    TaxTotal,
    Metadata,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(Iden)]
enum CartLineItems {
    Table,
    Id,
    CartId,
    ProductId,
    VariantId,
    FulfillmentRequirement,
    ShippingProfileSlug,
    Sku,
    Quantity,
    UnitPrice,
    TotalPrice,
    CurrencyCode,
    Metadata,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CartAdjustments {
    Table,
    Id,
    CartId,
    CartLineItemId,
    SourceType,
    SourceId,
    Amount,
    CurrencyCode,
    Metadata,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CartTaxLines {
    Table,
    Id,
    CartId,
    CartLineItemId,
    ShippingOptionId,
    Description,
    ProviderId,
    Rate,
    Amount,
    CurrencyCode,
    Metadata,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CartLineItemTranslations {
    Table,
    Id,
    CartLineItemId,
    Locale,
    Title,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CartShippingSelections {
    Table,
    Id,
    CartId,
    ShippingProfileSlug,
    SellerId,
    SellerScope,
    SelectedShippingOptionId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CartLineItemMarketplaceSnapshots {
    Table,
    CartLineItemId,
    SellerId,
    ListingId,
    MasterProductId,
    MasterVariantId,
    ListingTermsVersion,
    CurrencyCode,
    CurrencyExponent,
    UnitAmount,
    SubtotalAmount,
    DiscountAmount,
    TaxAmount,
    TotalAmount,
    PricingReference,
    InventoryReference,
    FulfillmentProfileSlug,
    CreatedAt,
    UpdatedAt,
}
