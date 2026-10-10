# Newsletter Module Implementation Plan

## Module Identity

- **Slug:** `newsletter`
- **Crate:** `rustok-newsletter`
- **API crate:** `rustok-newsletter-api`
- **Kind:** Optional
- **Category:** Communication / Marketing

## FFA/FBA Status

| Dimension | Status |
|---|---|
| FFA/FBA gate | ✅ Passed |
| Canonical domain contract | ✅ Defined |
| Request context | ✅ `PortContext` from `rustok-api` |
| Data ownership | ✅ 3 tables owned |
| Cross-module ports | ✅ ContentProvider, SubscriberPort, CampaignPort |
| Central board row | ⏳ Pending |
| Transport adapters | ✅ GraphQL complete |
| Module-owned UI | ⏳ Pending (admin package) |

## Release and Data Rollback Readiness

| Field | Value |
|---|---|
| Runtime kind | Optional |
| Rollback unit | Component |
| Data boundary owner | `newsletter_subscribers`, `newsletter_campaigns`, `newsletter_subscriptions` |
| Native migrations | `m20261009_000040_create_newsletter_tables` (AdditiveOnly) |
| Supported migration policy | AdditiveOnly |
| Predecessor standby strategy | none |
| Rollback eligibility | AutomaticSingleAttempt |
| Recovery invariants | Monotonic epoch, idempotency receipts, zero-flapping |

## Implementation Phases

### Phase 1: Core Infrastructure ✅

- [x] API crate with contracts
- [x] Domain state machines (subscriber, campaign)
- [x] DTO layer
- [x] Entity mappings
- [x] Migrations
- [x] Services (subscriber, campaign)
- [x] Ports implementation
- [x] GraphQL layer
- [x] Module registration
- [x] Workspace integration

### Phase 2: Content Providers ⏳

- [ ] Blog content provider implementation
- [ ] Forum content provider implementation
- [ ] Commerce content provider implementation
- [ ] Content provider registry wiring
- [ ] Content aggregation service

### Phase 3: Delivery System ⏳

- [ ] Email template system integration
- [ ] Campaign renderer (content → HTML email)
- [ ] Email delivery via `EmailDeliveryPort`
- [ ] Delivery tracking and receipts
- [ ] Bounce/suppression handling

### Phase 4: Scheduler ⏳

- [ ] Campaign scheduler worker
- [ ] Scheduled campaign pickup
- [ ] Batch sending with rate limiting
- [ ] Progress tracking
- [ ] Failure recovery

### Phase 5: Admin UI ⏳

- [ ] `rustok-newsletter-admin` package
- [ ] Subscriber management UI
- [ ] Campaign builder UI
- [ ] Campaign preview
- [ ] Analytics dashboard

### Phase 6: Public API ⏳

- [ ] Public subscription endpoint (storefront)
- [ ] Double opt-in confirmation flow
- [ ] Unsubscribe page
- [ ] Preference center

## Cross-Module Integration Points

### Blog → Newsletter

```rust
// rustok-blog implements NewsletterContentProvider
impl NewsletterContentProvider for BlogContentProvider {
    fn source_slug(&self) -> ContentSourceSlug {
        ContentSourceSlug::new("blog").unwrap()
    }
    
    async fn fetch_content(&self, request: ContentFetchRequest) -> Result<Vec<NewsletterContentItem>, NewsletterApiError> {
        // Fetch recent published posts
    }
}
```

### Forum → Newsletter

```rust
// rustok-forum implements NewsletterContentProvider
impl NewsletterContentProvider for ForumContentProvider {
    fn source_slug(&self) -> ContentSourceSlug {
        ContentSourceSlug::new("forum").unwrap()
    }
    
    async fn fetch_content(&self, request: ContentFetchRequest) -> Result<Vec<NewsletterContentItem>, NewsletterApiError> {
        // Fetch hot topics
    }
}
```

### Commerce → Newsletter

```rust
// rustok-commerce implements NewsletterContentProvider
impl NewsletterContentProvider for CommerceContentProvider {
    fn source_slug(&self) -> ContentSourceSlug {
        ContentSourceSlug::new("commerce").unwrap()
    }
    
    async fn fetch_content(&self, request: ContentFetchRequest) -> Result<Vec<NewsletterContentItem>, NewsletterApiError> {
        // Fetch new products, promotions
    }
}
```

## Testing Strategy

### Unit Tests

- Domain state machines (subscriber/campaign transitions)
- Validation logic
- Error mapping

### Integration Tests

- Subscriber CRUD operations
- Campaign lifecycle
- Port implementations
- GraphQL resolvers

### Contract Tests

- Content provider contract compliance
- Port contract compliance
- Migration safety

## Security Considerations

- Email addresses stored in lowercase, trimmed
- Confirmation tokens are cryptographically random
- Subscriber data is tenant-isolated
- Campaign operations require appropriate permissions
- PII handling follows platform privacy policy

## Performance Considerations

- Indexed queries for common access patterns
- Paginated list operations
- Batch sending with rate limiting (future)
- Content caching (future)

## Monitoring and Observability

- Structured logging with tracing
- Campaign delivery metrics (future)
- Subscriber growth metrics (future)
- Error rate tracking

## Dependencies

### Required

- `rustok-email` — email transport
- `rustok-outbox` — transactional events

### Optional

- `rustok-blog` — blog content provider
- `rustok-forum` — forum content provider
- `rustok-commerce` — commerce content provider

## Notes

This module follows the canonical native module source layout as defined in the platform architecture. It is designed as a standalone bounded context, not a sub-feature of any source module, enabling cross-domain newsletter functionality.
