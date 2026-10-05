        "tracking_number": "TRACK-1",
        "metadata": {}
    });
    let wrong_provider_error = journal
        .mark_provider_succeeded(
            tenant_id,
            operation.id,
            Some("shipment-1".to_string()),
            wrong_provider,
        )
        .await
        .expect_err("journal writer must reject a result for another provider");
    assert!(matches!(
        wrong_provider_error,
        rustok_fulfillment::error::FulfillmentError::ProviderResultInvalid(_)
    ));

    let wrong_reference = serde_json::json!({
        "provider_id": "carrier",
        "external_reference": "shipment-2",
        "tracking_number": "TRACK-1",
        "metadata": {}
    });
    let wrong_reference_error = journal
        .mark_provider_succeeded(
            tenant_id,
            operation.id,
            Some("shipment-1".to_string()),
            wrong_reference,
        )
        .await
        .expect_err("journal writer must reject mismatched provider reference");
    assert!(matches!(
        wrong_reference_error,
        rustok_fulfillment::error::FulfillmentError::ProviderResultInvalid(_)
    ));

    let scalar_metadata_error = journal
        .mark_provider_succeeded(
            tenant_id,
            operation.id,
            Some("shipment-1".to_string()),
            serde_json::json!({
                "provider_id": "carrier",
                "external_reference": "shipment-1",
                "tracking_number": "TRACK-1",
                "metadata": "not-an-object"
            }),
        )
        .await
        .expect_err("journal writer must reject non-object provider metadata");
    assert!(matches!(
        scalar_metadata_error,
        rustok_fulfillment::error::FulfillmentError::ProviderResultInvalid(_)
    ));

    let oversized_reference_error = journal
        .mark_provider_succeeded(
            tenant_id,
            operation.id,
            Some("r".repeat(192)),
            serde_json::json!({
                "provider_id": "carrier",
                "external_reference": null,
                "tracking_number": "TRACK-1",
                "metadata": {}
            }),
        )
        .await
        .expect_err("journal writer must reject oversized provider references");
    assert!(matches!(
        oversized_reference_error,
        rustok_fulfillment::error::FulfillmentError::ProviderResultInvalid(_)
    ));

    let unresolved_result = journal
        .mark_execution_reconciliation_required(
            tenant_id,
            operation.id,
            Some("shipment-1".to_string()),
            Some(serde_json::json!({
                "provider_id": "other-carrier",
                "external_reference": "shipment-1",
                "tracking_number": "TRACK-1",
                "metadata": {}
            })),
            "provider result identity mismatch",
        )
        .await
        .expect_err("reconciliation writer must reject a mismatched provider result");
    assert!(matches!(
        unresolved_result,
        rustok_fulfillment::error::FulfillmentError::ProviderResultInvalid(_)
    ));

    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("operation should remain readable");
    assert_eq!(
        current.status,
        rustok_fulfillment::PROVIDER_OPERATION_EXECUTING,
        "identity rejection must not destroy the unresolved execution state"
    );