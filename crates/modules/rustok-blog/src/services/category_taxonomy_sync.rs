/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use crate::error::BlogResult;
use crate::ports::taxonomy::{
    BlogTaxonomyCategoryLocaleCopy, BlogTaxonomyPort, BlogTaxonomySyncCategoryInput,
};

pub(crate) async fn load_category_locale_copy_in_tx(
    port: &dyn BlogTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
) -> BlogResult<Option<BlogTaxonomyCategoryLocaleCopy>> {
    port.load_category_locale_copy_in_tx(txn, tenant_id, category_id, locale)
        .await
}

pub(crate) async fn sync_category_copy_in_tx(
    port: &dyn BlogTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    parent_id: Option<Uuid>,
    position: i32,
    locale: String,
    name: String,
    slug: String,
    description: Option<String>,
) -> BlogResult<()> {
    port.sync_category_copy_in_tx(
        txn,
        tenant_id,
        BlogTaxonomySyncCategoryInput {
            category_id,
            parent_id,
            position,
            locale,
            name,
            slug,
            description,
        },
    )
    .await
}
