use sea_orm_migration::prelude::*;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260723_000002_create_iggy_connector_settings::Migration,
    )]
}

mod m20260723_000002_create_iggy_connector_settings {
    use sea_orm_migration::prelude::*;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .create_table(
                    Table::create()
                        .table(IggyConnectorSettings::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(IggyConnectorSettings::Id)
                                .integer()
                                .not_null()
                                .primary_key(),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::Mode)
                                .string_len(16)
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::ExternalAddresses)
                                .json()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::ExternalUsername)
                                .string_len(255)
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::PasswordResolver)
                                .string_len(64)
                                .null(),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::PasswordKey)
                                .string_len(512)
                                .null(),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::SecretTenantId)
                                .uuid()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::TlsEnabled)
                                .boolean()
                                .not_null()
                                .default(false),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::TlsDomain)
                                .string_len(255)
                                .null(),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::UpdatedBy)
                                .uuid()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::CreatedAt)
                                .timestamp_with_time_zone()
                                .not_null()
                                .default(Expr::current_timestamp()),
                        )
                        .col(
                            ColumnDef::new(IggyConnectorSettings::UpdatedAt)
                                .timestamp_with_time_zone()
                                .not_null()
                                .default(Expr::current_timestamp()),
                        )
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(IggyConnectorSettings::Table).to_owned())
                .await
        }
    }

    #[derive(Iden)]
    enum IggyConnectorSettings {
        Table,
        Id,
        Mode,
        ExternalAddresses,
        ExternalUsername,
        PasswordResolver,
        PasswordKey,
        SecretTenantId,
        TlsEnabled,
        TlsDomain,
        UpdatedBy,
        CreatedAt,
        UpdatedAt,
    }
}
