/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use crate::scoped_caller_idempotency_key;
use crate::services::receipts::complete_relation_operation;
use crate::services::{ProductRelationCommandContext, ProductRelationCommandError};
use rustok_outbox::{SysEventsMigration, idempotency};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement, TransactionTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

#[test]
fn deployed_relation_receipt_key_formula_stays_stable() {
    let key = scoped_caller_idempotency_key(
        Uuid::from_u128(0x11111111111111111111111111111111),
        Uuid::from_u128(0x22222222222222222222222222222222),
        "add_product_relation",
        None,
        " retry-123 ",
    );

    assert_eq!(
        key,
        "commerce-graphql-relation:add_product_relation:8346a5949e6c96a2844f67fbc37f4d112a3dd36bec196ef4ca8f9c61422ef7af"
    );
}

#[tokio::test]
async fn receipt_completion_failure_rolls_back_staged_relation_mutation() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("database should connect");
    SysEventsMigration
        .up(&SchemaManager::new(&database))
        .await
        .expect("outbox migration should apply");
    database
        .execute_unprepared("CREATE TABLE relation_receipt_probe (id INTEGER PRIMARY KEY)")
        .await
        .expect("probe table should exist");

    let transaction = database.begin().await.expect("transaction should begin");
    transaction
        .execute_unprepared("INSERT INTO relation_receipt_probe (id) VALUES (1)")
        .await
        .expect("probe mutation should stage");

    let error = complete_relation_operation(
        transaction,
        ProductRelationCommandContext::new(Uuid::new_v4(), None, "receipt-key"),
        idempotency::Lease {
            operation_id: Uuid::new_v4(),
            token: Uuid::new_v4(),
        },
        &serde_json::json!({"ok": true}),
    )
    .await
    .expect_err("an unknown receipt lease must reject completion");
    assert!(matches!(
        error,
        ProductRelationCommandError::Receipt(ref error)
            if error.code == "outbox.operation_receipt_state_invalid"
    ));

    let row = database
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM relation_receipt_probe".to_owned(),
        ))
        .await
        .expect("probe count should query")
        .expect("probe count row should exist");
    let count: i64 = row.try_get("", "count").expect("probe count should decode");
    assert_eq!(
        count, 0,
        "completion failure must await transaction rollback"
    );
}
