use std::{
    future::pending,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext};
use rustok_index::{
    FieldName, FieldPath, IndexProjectedValue, IndexQueryItem, IndexQueryPage, IndexValue,
};
use rustok_product::{
    ProductStorefrontTagHydration, ProductStorefrontTagHydrationItem, StorefrontProductList,
    StorefrontProductListItem, StorefrontProductListQuery, entities::product::ProductStatus,
};
use uuid::Uuid;

use super::{
    ProductStorefrontIndexBudgetedProjectionError, ProductStorefrontIndexBudgetedProjectionExecutor,
    ProductStorefrontIndexBudgetedStartError, ProductStorefrontIndexBudgetedTagHydrationError,
    ProductStorefrontIndexProjectionPhases, ProductStorefrontIndexServingBudgetDecision,
    ProductStorefrontIndexShadowProjectionError, ProductStorefrontIndexTagHydrationError,
};

#[derive(Clone, Copy)]
enum IndexBehavior {
    Return,
    Error,
    Pending,
}

#[derive(Clone, Copy)]
enum TagBehavior {
    Return,
    Pending,
}

#[derive(Default)]
struct FakeState {
    index_calls: AtomicUsize,
    tag_calls: AtomicUsize,
    index_deadlines: Mutex<Vec<Option<u64>>>,
    tag_deadlines: Mutex<Vec<Option<u64>>>,
}

struct FakePhases {
    state: Arc<FakeState>,
    index_behavior: IndexBehavior,
    tag_behavior: TagBehavior,
    page: IndexQueryPage,
    hydration: ProductStorefrontTagHydration,
}

#[async_trait]
impl ProductStorefrontIndexProjectionPhases for FakePhases {
    async fn execute_projected(
        &self,
        context: PortContext,
        _fallback_locale: String,
        _public_channel_slug: Option<String>,
        _public_channel_id: Option<Uuid>,
        _query: StorefrontProductListQuery,
    ) -> Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError> {
        self.state.index_calls.fetch_add(1, Ordering::SeqCst);
        self.state
            .index_deadlines
            .lock()
            .unwrap()
            .push(context.deadline_ms);
        match self.index_behavior {
            IndexBehavior::Return => Ok(self.page.clone()),
            IndexBehavior::Error => Err(ProductStorefrontIndexShadowProjectionError::InvalidTenant),
            IndexBehavior::Pending => {
                pending::<()>().await;
                unreachable!("pending projected phase is cancelled only by the budget timeout")
            }
        }
    }

    async fn hydrate_projected_tags(
        &self,
        context: PortContext,
        _fallback_locale: String,
        _projected: &IndexQueryPage,
    ) -> Result<ProductStorefrontTagHydration, ProductStorefrontIndexTagHydrationError> {
        self.state.tag_calls.fetch_add(1, Ordering::SeqCst);
        self.state
            .tag_deadlines
            .lock()
            .unwrap()
            .push(context.deadline_ms);
        match self.tag_behavior {
            TagBehavior::Return => Ok(self.hydration.clone()),
            TagBehavior::Pending => {
                pending::<()>().await;
                unreachable!("pending tag phase is cancelled only by the budget timeout")
            }
        }
    }
}

fn context() -> PortContext {
    PortContext::new(
        Uuid::from_u128(0x9100).to_string(),
        PortActor::system(),
        "fi",
        "budgeted-storefront-evidence",
    )
}

fn field(name: &str, value: IndexValue) -> IndexProjectedValue {
    IndexProjectedValue {
        path: FieldPath::new(FieldName::new(name).unwrap()),
        value,
    }
}

fn raw_page(product_id: Uuid) -> IndexQueryPage {
    IndexQueryPage {
        items: vec![IndexQueryItem {
            entity_id: product_id,
            relations: Vec::new(),
            fields: vec![
                field("title", IndexValue::Null),
                field("handle", IndexValue::Null),
                field("tag_ids", IndexValue::List(Vec::new())),
            ],
            nested_relations: Vec::new(),
        }],
        exact_count: Some(1),
        has_more: false,
        next_cursor: None,
    }
}

fn authoritative_page(product_id: Uuid) -> StorefrontProductList {
    StorefrontProductList {
        items: vec![StorefrontProductListItem {
            id: product_id,
            status: ProductStatus::Active,
            title: "Untitled product".to_owned(),
            handle: String::new(),
            seller_id: None,
            vendor: None,
            product_type: None,
            tags: vec!["Legacy".to_owned()],
            created_at: chrono::Utc::now(),
            published_at: None,
        }],
        total: 1,
        page: 1,
        per_page: 12,
        has_next: false,
    }
}

fn hydration(product_id: Uuid) -> ProductStorefrontTagHydration {
    ProductStorefrontTagHydration {
        items: vec![ProductStorefrontTagHydrationItem {
            product_id,
            tags: vec!["Legacy".to_owned()],
        }],
    }
}

fn fake_executor(
    product_id: Uuid,
    index_behavior: IndexBehavior,
    tag_behavior: TagBehavior,
) -> (ProductStorefrontIndexBudgetedProjectionExecutor, Arc<FakeState>) {
    let state = Arc::new(FakeState::default());
    let phases = FakePhases {
        state: state.clone(),
        index_behavior,
        tag_behavior,
        page: raw_page(product_id),
        hydration: hydration(product_id),
    };
    (
        ProductStorefrontIndexBudgetedProjectionExecutor::from_phases(Arc::new(phases)),
        state,
    )
}

fn eligible(index_execution_ms: u64, tag_hydration_ms: u64) -> ProductStorefrontIndexServingBudgetDecision {
    ProductStorefrontIndexServingBudgetDecision::Eligible {
        index_execution_ms,
        tag_hydration_ms,
        safety_margin_ms: 1,
    }
}

async fn execute(
    executor: &ProductStorefrontIndexBudgetedProjectionExecutor,
    authoritative: StorefrontProductList,
    decision: ProductStorefrontIndexServingBudgetDecision,
) -> Result<super::ProductStorefrontIndexBudgetedExecution, ProductStorefrontIndexBudgetedStartError> {
    executor
        .execute_after_owner(
            authoritative,
            context(),
            "en".to_owned(),
            Some("online".to_owned()),
            Some(Uuid::from_u128(0x9200)),
            StorefrontProductListQuery::default(),
            decision,
        )
        .await
}

fn projected_string(page: &IndexQueryPage, field_name: &str) -> Option<&str> {
    let item = page.items.first()?;
    item.fields.iter().find_map(|projected| {
        (projected.path.links().is_empty() && projected.path.field().as_str() == field_name)
            .then_some(&projected.value)
            .and_then(|value| match value {
                IndexValue::String(value) => Some(value.as_str()),
                _ => None,
            })
    })
}

#[tokio::test]
async fn noneligible_budget_starts_no_projected_work() {
    let product_id = Uuid::from_u128(0x9301);
    let (executor, state) = fake_executor(product_id, IndexBehavior::Return, TagBehavior::Return);
    let outcome = execute(
        &executor,
        authoritative_page(product_id),
        ProductStorefrontIndexServingBudgetDecision::OwnerNativeInsufficientBudget {
            required_ms: 10,
            remaining_ms: 9,
        },
    )
    .await;

    assert!(matches!(
        outcome,
        Err(ProductStorefrontIndexBudgetedStartError::BudgetNotEligible(
            ProductStorefrontIndexServingBudgetDecision::OwnerNativeInsufficientBudget { .. }
        ))
    ));
    assert_eq!(state.index_calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.tag_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn index_timeout_preserves_authoritative_owner_page_and_skips_enrichment() {
    let product_id = Uuid::from_u128(0x9302);
    let authoritative = authoritative_page(product_id);
    let (executor, state) = fake_executor(product_id, IndexBehavior::Pending, TagBehavior::Return);
    let execution = execute(&executor, authoritative.clone(), eligible(1, 20))
        .await
        .unwrap();

    assert_eq!(execution.authoritative.items[0].id, authoritative.items[0].id);
    assert!(matches!(
        execution.projected,
        Err(ProductStorefrontIndexBudgetedProjectionError::TimedOut { budget_ms: 1 })
    ));
    assert!(execution.public_projected.is_none());
    assert!(execution.tag_hydration.is_none());
    assert!(execution.comparison.is_none());
    assert_eq!(state.index_calls.load(Ordering::SeqCst), 1);
    assert_eq!(state.tag_calls.load(Ordering::SeqCst), 0);
    assert_eq!(*state.index_deadlines.lock().unwrap(), vec![Some(1)]);
}

#[tokio::test]
async fn raw_projection_failure_skips_public_projection_and_tag_hydration() {
    let product_id = Uuid::from_u128(0x9303);
    let (executor, state) = fake_executor(product_id, IndexBehavior::Error, TagBehavior::Return);
    let execution = execute(&executor, authoritative_page(product_id), eligible(20, 20))
        .await
        .unwrap();

    assert!(matches!(
        execution.projected,
        Err(ProductStorefrontIndexBudgetedProjectionError::Projection(
            ProductStorefrontIndexShadowProjectionError::InvalidTenant
        ))
    ));
    assert!(execution.public_projected.is_none());
    assert!(execution.tag_hydration.is_none());
    assert!(execution.comparison.is_none());
    assert_eq!(state.index_calls.load(Ordering::SeqCst), 1);
    assert_eq!(state.tag_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn tag_timeout_preserves_raw_public_pages_and_phase_deadlines() {
    let product_id = Uuid::from_u128(0x9304);
    let authoritative = authoritative_page(product_id);
    let (executor, state) = fake_executor(product_id, IndexBehavior::Return, TagBehavior::Pending);
    let execution = execute(&executor, authoritative.clone(), eligible(40, 1))
        .await
        .unwrap();

    assert_eq!(execution.authoritative.items[0].id, product_id);
    assert!(execution.projected.is_ok());
    let public = execution.public_projected.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(projected_string(public, "title"), Some("Untitled product"));
    assert_eq!(projected_string(public, "handle"), Some(""));
    assert!(matches!(
        execution.tag_hydration,
        Some(Err(ProductStorefrontIndexBudgetedTagHydrationError::TimedOut { budget_ms: 1 }))
    ));
    assert!(execution.comparison.unwrap().is_match());
    assert_eq!(*state.index_deadlines.lock().unwrap(), vec![Some(40)]);
    assert_eq!(*state.tag_deadlines.lock().unwrap(), vec![Some(1)]);
}

#[tokio::test]
async fn eligible_fast_path_preserves_identity_count_and_owner_tag_projection() {
    let product_id = Uuid::from_u128(0x9305);
    let (executor, state) = fake_executor(product_id, IndexBehavior::Return, TagBehavior::Return);
    let execution = execute(&executor, authoritative_page(product_id), eligible(50, 30))
        .await
        .unwrap();

    let raw = execution.projected.as_ref().unwrap();
    assert_eq!(raw.items.iter().map(|item| item.entity_id).collect::<Vec<_>>(), vec![product_id]);
    assert_eq!(raw.exact_count, Some(1));
    assert!(!raw.has_more);
    assert!(execution.comparison.unwrap().is_match());
    assert_eq!(execution.index_execution_budget_ms, 50);
    assert_eq!(execution.tag_hydration_budget_ms, 30);
    assert_eq!(execution.safety_margin_ms, 1);

    let public = execution.public_projected.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(projected_string(public, "title"), Some("Untitled product"));
    assert_eq!(projected_string(public, "handle"), Some(""));

    let hydrated = execution.tag_hydration.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(hydrated.items.len(), 1);
    assert_eq!(hydrated.items[0].product_id, product_id);
    assert_eq!(hydrated.items[0].tags, vec!["Legacy".to_owned()]);
    assert_eq!(*state.index_deadlines.lock().unwrap(), vec![Some(50)]);
    assert_eq!(*state.tag_deadlines.lock().unwrap(), vec![Some(30)]);
}
