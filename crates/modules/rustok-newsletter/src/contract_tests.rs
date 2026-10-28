use rustok_core::module::{ModuleKind, RusToKModule};

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public surface",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[test]
fn module_metadata_matches_contract() {
    let module = crate::NewsletterModule::new();

    assert_eq!(module.slug(), "newsletter");
    assert_eq!(module.name(), "Newsletter");
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert_eq!(module.dependencies(), &["email", "outbox"]);
}

#[tokio::test]
async fn newsletter_service_nil_uuid_guards() {
    use crate::dto::{CreateCampaignInput, SubscribeInput};
    use crate::services::{CampaignService, SubscriberService};
    use sea_orm::Database;
    use uuid::Uuid;

    let db = Database::connect("sqlite::memory:").await.unwrap();
    let subscriber_service = SubscriberService::new(db.clone());
    let campaign_service = CampaignService::new(db);

    let err = subscriber_service
        .subscribe(
            Uuid::nil(),
            SubscribeInput {
                email: "test@example.com".to_string(),
                name: None,
                locale: None,
            },
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Tenant ID must not be nil"));

    let err = subscriber_service
        .subscribe(
            Uuid::new_v4(),
            SubscribeInput {
                email: "invalid-email".to_string(),
                name: None,
                locale: None,
            },
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Invalid email address"));

    let err = subscriber_service
        .confirm(Uuid::nil(), Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Tenant ID must not be nil"));

    let err = subscriber_service
        .confirm(Uuid::new_v4(), Uuid::nil())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Subscriber ID must not be nil"));

    let err = campaign_service
        .create(
            Uuid::nil(),
            CreateCampaignInput {
                title: "Title".to_string(),
                subject: "Subject".to_string(),
                preheader: None,
                content_sources: vec![],
                segment_id: None,
            },
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Tenant ID must not be nil"));

    let err = campaign_service
        .create(
            Uuid::new_v4(),
            CreateCampaignInput {
                title: "   ".to_string(),
                subject: "Subject".to_string(),
                preheader: None,
                content_sources: vec![],
                segment_id: None,
            },
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Title cannot be empty"));

    let err = campaign_service
        .schedule(
            Uuid::nil(),
            Uuid::new_v4(),
            crate::dto::ScheduleCampaignInput {
                scheduled_at: chrono::Utc::now(),
            },
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Tenant ID must not be nil"));
}
