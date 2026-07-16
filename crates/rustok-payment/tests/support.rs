use rustok_payment::entities::{payment, payment_collection, refund_creation};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Schema};

pub async fn ensure_payment_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(payment_collection::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(payment::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(refund_creation::Entity),
    )
    .await;

    db.execute_unprepared(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS ux_payment_collections_active_cart
        ON payment_collections (tenant_id, cart_id)
        WHERE cart_id IS NOT NULL
          AND status IN ('pending', 'authorized', 'captured')
        "#,
    )
    .await
    .expect("active cart payment collection index should be created");
    db.execute_unprepared(
        r#"
        CREATE TRIGGER IF NOT EXISTS payment_collections_order_binding_guard
        BEFORE UPDATE OF order_id ON payment_collections
        FOR EACH ROW
        WHEN OLD.order_id IS NOT NULL
         AND NEW.order_id IS NOT OLD.order_id
        BEGIN
            SELECT RAISE(ABORT, 'payment collection order binding is immutable');
        END;
        "#,
    )
    .await
    .expect("payment collection order binding trigger should be created");
    db.execute_unprepared(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS ux_refunds_creation_identity
        ON refunds (tenant_id, payment_collection_id, creation_key);

        CREATE TRIGGER IF NOT EXISTS refunds_creation_identity_guard_insert
        BEFORE INSERT ON refunds
        FOR EACH ROW
        BEGIN
            SELECT CASE WHEN NEW.creation_key IS NULL
                OR trim(NEW.creation_key) = ''
                OR NEW.creation_request_hash IS NULL
                OR length(NEW.creation_request_hash) <> 64
                OR NEW.creation_request_hash GLOB '*[^0-9a-f]*'
                THEN RAISE(ABORT, 'refund creation identity is required') END;
        END;

        CREATE TRIGGER IF NOT EXISTS refunds_creation_identity_guard_update
        BEFORE UPDATE ON refunds
        FOR EACH ROW
        BEGIN
            SELECT CASE WHEN NEW.creation_key IS NOT OLD.creation_key
                OR NEW.creation_request_hash IS NOT OLD.creation_request_hash
                THEN RAISE(ABORT, 'refund creation identity is immutable') END;
        END;
        "#,
    )
    .await
    .expect("refund creation identity guards should be created");
}

pub(crate) async fn create_entity_table(
    db: &DatabaseConnection,
    builder: &DbBackend,
    mut statement: sea_orm::sea_query::TableCreateStatement,
) {
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("failed to create payment test table");
}
