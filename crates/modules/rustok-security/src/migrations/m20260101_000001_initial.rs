/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. security_ip_rules
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("security_ip_rules"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("ip_or_cidr")).string_len(64).not_null())
                    .col(ColumnDef::new(Alias::new("rule_type")).string_len(32).not_null())
                    .col(ColumnDef::new(Alias::new("reason")).string_len(255).null())
                    .col(ColumnDef::new(Alias::new("expires_at")).timestamp_with_time_zone().null())
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
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_security_ip_rules_tenant_type")
                    .table(Alias::new("security_ip_rules"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("rule_type"))
                    .to_owned(),
            )
            .await?;

        // 2. security_audit_logs
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("security_audit_logs"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("actor_id")).uuid().null())
                    .col(ColumnDef::new(Alias::new("ip_address")).string_len(64).not_null())
                    .col(ColumnDef::new(Alias::new("user_agent")).string_len(512).null())
                    .col(ColumnDef::new(Alias::new("event_type")).string_len(64).not_null())
                    .col(ColumnDef::new(Alias::new("severity")).string_len(32).not_null())
                    .col(ColumnDef::new(Alias::new("details")).json().not_null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_security_audit_logs_tenant_created")
                    .table(Alias::new("security_audit_logs"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("created_at"))
                    .to_owned(),
            )
            .await?;

        // 3. security_policies
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("security_policies"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("tenant_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("max_failed_login_attempts")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("lockout_duration_secs")).big_integer().not_null())
                    .col(ColumnDef::new(Alias::new("rate_limit_per_minute")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("captcha_provider")).string_len(32).not_null())
                    .col(ColumnDef::new(Alias::new("captcha_site_key")).string_len(255).null())
                    .col(ColumnDef::new(Alias::new("captcha_secret_key")).string_len(255).null())
                    .col(ColumnDef::new(Alias::new("mfa_enforced_roles")).json().not_null())
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("security_policies")).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Alias::new("security_audit_logs")).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Alias::new("security_ip_rules")).to_owned())
            .await
    }
}
