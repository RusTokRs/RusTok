use super::*;
use rustok_core::MigrationSource;
use sea_orm_migration::SchemaManager;

#[tokio::test]
async fn cart_without_region_recalculates_without_region_schema() {
    let (db, service) = setup_with_db().await;
    let schema = SchemaManager::new(&db);
    // Region is optional for an unscoped Cart. Remove its entire schema via
    // the owner migration chain to catch accidental reads in that branch.
    for migration in rustok_region::RegionModule.migrations().into_iter().rev() {
        migration.down(&schema).await.unwrap();
    }
    assert!(!schema.has_table("regions").await.unwrap());

    let tenant_id = support::TEST_TENANT_ID;
    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let saved = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();
    assert_eq!(saved.subtotal_amount, Decimal::from(31));
    assert_eq!(saved.total_amount, Decimal::from(31));
    assert_eq!(saved.tax_total, Decimal::ZERO);
    assert!(saved.tax_lines.is_empty());

    let persisted = service.get_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(persisted.line_items[0].id, saved.line_items[0].id);
    assert_eq!(persisted.total_amount, saved.total_amount);
}

#[tokio::test]
async fn region_policy_missing_or_foreign_rolls_back_cart_line() {
    for foreign in [false, true] {
        let (db, service) = setup_with_db().await;
        let tenant_id = support::TEST_TENANT_ID;
        let region_id = if foreign {
            insert_region(
                &db,
                Uuid::new_v4(),
                "USD",
                "US",
                None,
                serde_json::json!({}),
            )
            .await
        } else {
            Uuid::new_v4()
        };
        let cart = service
            .create_cart(
                tenant_id,
                CreateCartInput {
                    region_id: Some(region_id),
                    ..create_cart_input()
                },
            )
            .await
            .unwrap();
        let result = service
            .add_line_item(tenant_id, cart.id, line_item_input())
            .await;
        assert!(
            matches!(result, Err(CartError::Validation(message)) if message == "Region not found for cart")
        );
        let persisted = service.get_cart(tenant_id, cart.id).await.unwrap();
        assert!(persisted.line_items.is_empty());
        assert!(persisted.tax_lines.is_empty());
        assert_eq!(persisted.total_amount, Decimal::ZERO);
    }
}

#[tokio::test]
async fn region_policy_storage_failure_preserves_cart_and_tax_snapshot() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = insert_region(&db, tenant_id, "USD", "US", None, serde_json::json!({})).await;
    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
        )
        .await
        .unwrap();
    let saved = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();
    assert!(!saved.tax_lines.is_empty());

    // Remove the owner dependency only in this isolated fixture. The Cart
    // transaction has already written a line and deleted its previous tax
    // lines when the policy read fails.
    db.execute_unprepared("DROP TABLE region_country_tax_policies")
        .await
        .unwrap();
    assert!(matches!(
        service
            .add_line_item(tenant_id, cart.id, line_item_input())
            .await,
        Err(CartError::Database(_))
    ));
    let persisted = service.get_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(persisted.line_items.len(), saved.line_items.len());
    assert_eq!(
        persisted.line_items[0].quantity,
        saved.line_items[0].quantity
    );
    assert_eq!(persisted.tax_total, saved.tax_total);
    assert_eq!(persisted.total_amount, saved.total_amount);
    assert_eq!(persisted.tax_lines[0].id, saved.tax_lines[0].id);
}

#[tokio::test]
async fn region_owner_country_rule_controls_cart_tax_and_inclusion() {
    for included in [false, true] {
        let (db, service) = setup_with_db().await;
        let tenant_id = support::TEST_TENANT_ID;
        let region_id =
            insert_region(&db, tenant_id, "USD", "US", None, serde_json::json!({})).await;
        rustok_region::RegionService::new(db.clone())
            .update_region(
                tenant_id,
                region_id,
                rustok_region::UpdateRegionInput {
                    country_tax_policies: Some(vec![rustok_region::RegionCountryTaxPolicyInput {
                        country_code: "US".into(),
                        tax_rate: Decimal::from(7),
                        tax_included: included,
                    }]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let cart = service
            .create_cart(
                tenant_id,
                CreateCartInput {
                    region_id: Some(region_id),
                    country_code: Some("US".into()),
                    ..create_cart_input()
                },
            )
            .await
            .unwrap();
        let cart = service
            .add_line_item(
                tenant_id,
                cart.id,
                AddCartLineItemInput {
                    unit_price: Decimal::from(100),
                    quantity: 1,
                    ..line_item_input()
                },
            )
            .await
            .unwrap();
        let (expected_tax, expected_total) = if included {
            (Decimal::new(654, 2), Decimal::from(100))
        } else {
            (Decimal::from(7), Decimal::from(107))
        };
        assert_eq!(cart.tax_total, expected_tax);
        assert_eq!(cart.total_amount, expected_total);
        assert_eq!(cart.tax_lines[0].rate, Decimal::from(7));
        assert_eq!(cart.tax_lines[0].provider_id, "region_default");
    }
}

#[tokio::test]
async fn channel_tax_provider_mapping_overrides_region_provider() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let channel_id = Uuid::new_v4();

    let region_id = insert_region(
        &db,
        tenant_id,
        "usd",
        "US",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "external_tax"
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let error = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect_err("unknown channel provider should fail");

    match error {
        CartError::TaxBoundary { code, message, .. } => {
            assert_eq!(code, "tax.validation");
            assert!(
                message.contains("tax request is invalid")
                    || message.contains("tax calculation request is invalid")
                    || message.contains("unknown tax provider_id: external_tax")
            );
        }
        other => panic!("expected tax validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn channel_mapping_is_ignored_without_cart_channel_context() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let mapped_channel_id = Uuid::new_v4();

    let region_id = insert_region(
        &db,
        tenant_id,
        "usd",
        "US",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                mapped_channel_id.to_string(): "external_tax"
            }
        }),
    )
    .await;

    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("channel mapping should not apply without cart channel context");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
}

#[tokio::test]
async fn channel_tax_provider_mapping_is_normalized_and_overrides_region_provider() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let channel_id = Uuid::new_v4();

    let region_id = insert_region(
        &db,
        tenant_id,
        "usd",
        "US",
        Some("external_tax"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "  REGION_DEFAULT  "
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("normalized channel mapping should override invalid region provider");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
}

#[tokio::test]
async fn object_channel_tax_provider_mapping_uses_provider_id_successfully() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let channel_id = Uuid::new_v4();

    let region_id = insert_region(
        &db,
        tenant_id,
        "usd",
        "US",
        Some("external_tax"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"provider_id": "region_default"}
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("provider_id object mapping should override invalid region provider");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
}

#[tokio::test]
async fn object_channel_tax_provider_mapping_uses_provider_key_alias() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let channel_id = Uuid::new_v4();

    let region_id = insert_region(
        &db,
        tenant_id,
        "usd",
        "US",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"provider": "external_tax"}
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect_err("unknown provider from object mapping should be validated");

    match updated {
        CartError::TaxBoundary { code, message, .. } => {
            assert_eq!(code, "tax.validation");
            assert!(
                message.contains("tax request is invalid")
                    || message.contains("tax calculation request is invalid")
                    || message.contains("unknown tax provider_id: external_tax")
            );
        }
        other => panic!("expected tax validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn channel_tax_provider_alias_is_ignored_without_channel_context() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let mapped_channel_id = Uuid::new_v4();

    let region_id = insert_region(
        &db,
        tenant_id,
        "usd",
        "US",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                mapped_channel_id.to_string(): {"provider": "external_tax"}
            }
        }),
    )
    .await;

    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("alias mapping should be ignored without channel context");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
    assert_eq!(
        updated.tax_lines[0].metadata["channel_id"],
        serde_json::json!(null)
    );
}

#[tokio::test]
async fn channel_tax_provider_alias_is_normalized_and_snapshots_channel_id() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let channel_id = Uuid::new_v4();

    let region_id = insert_region(
        &db,
        tenant_id,
        "usd",
        "US",
        Some("external_tax"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"provider": "  REGION_DEFAULT  "}
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("alias mapping should normalize and override invalid region provider");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
    assert_eq!(
        updated.tax_lines[0].metadata["channel_id"],
        serde_json::json!(channel_id.to_string())
    );
}

#[tokio::test]
async fn channel_tax_provider_mapping_with_invalid_chars_is_rejected() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let channel_id = Uuid::new_v4();

    let region_id = insert_region(
        &db,
        tenant_id,
        "usd",
        "US",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "INVALID PROVIDER"
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let error = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect_err("invalid provider id should be rejected");

    match error {
        CartError::TaxBoundary { code, message, .. } => {
            assert_eq!(code, "tax.validation");
            assert!(
                message.contains("tax request is invalid")
                    || message.contains("tax calculation request is invalid")
                    || message.contains("tax provider_id must use lowercase ASCII")
            );
        }
        other => panic!("expected tax validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn blank_channel_tax_provider_mapping_falls_back_to_region_provider() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let channel_id = Uuid::new_v4();

    let region_id = insert_region(
        &db,
        tenant_id,
        "usd",
        "US",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "   "
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("blank channel mapping should use region provider");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
}

#[tokio::test]
async fn tax_calculated_on_net_discounted_line_item_price() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;

    let region_id = insert_region(
        &db,
        tenant_id,
        "usd",
        "US",
        Some("region_default"),
        serde_json::json!({}),
    )
    .await;

    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
        )
        .await
        .unwrap();

    let cart = service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                unit_price: Decimal::from_str("100.00").unwrap(),
                quantity: 1,
                ..line_item_input()
            },
        )
        .await
        .unwrap();

    let line_item_id = cart.line_items[0].id;
    assert_eq!(cart.tax_total, Decimal::from_str("10.00").unwrap());

    let updated = service
        .set_adjustments(
            tenant_id,
            cart.id,
            vec![SetCartAdjustmentInput {
                line_item_id: Some(line_item_id),
                source_type: "promotion".to_string(),
                source_id: Some("promo-20".to_string()),
                amount: Decimal::from_str("20.00").unwrap(),
                metadata: serde_json::json!({}),
            }],
        )
        .await
        .unwrap();

    assert_eq!(updated.tax_total, Decimal::from_str("8.00").unwrap());
    assert_eq!(updated.total_amount, Decimal::from_str("88.00").unwrap());
}
