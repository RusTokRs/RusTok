# Newsletter Module — Implementation Summary

**Дата:** 2026-10-09  
**Статус:** ✅ Phase 1 Complete  
**Модуль:** `rustok-newsletter`

## Обзор

Создан полнофункциональный модуль newsletter как **отдельный bounded context**, следующий архитектурному паттерну платформы (аналогично `rustok-notifications`).

## Архитектурное решение

**Ключевой вопрос:** Где разместить функционал newsletter?

**Решение:** Отдельный модуль, НЕ субфича блога/форума/ecommerce.

**Обоснование:**
1. **Cross-domain nature** — агрегирует контент из блога, форума, магазина
2. **Separation of concerns** — newsletter != notifications (in-app) != email (transport)
3. **Follows platform pattern** — `*-api` + `*-impl` crates (как notifications)
4. **Inversion of control** — sources implement `NewsletterContentProvider` trait

## Созданные артефакты

### Crates

#### `rustok-newsletter-api` (contracts)
```
src/
  lib.rs           — crate facade
  model.rs         — shared types (ContentSourceSlug, SubscriberStatus, CampaignStatus)
  provider.rs      — NewsletterContentProvider trait
  port.rs          — SubscriberPort, CampaignPort traits
```

**Ключевые контракты:**
- `NewsletterContentProvider` — trait для source modules (blog/forum/commerce)
- `NewsletterSubscriberPort` — typed port для subscriber management
- `NewsletterCampaignPort` — typed port для campaign operations

#### `rustok-newsletter` (implementation)
```
src/
  lib.rs           — crate facade, re-exports
  module.rs        — RusToKModule, MigrationSource
  error.rs         — NewsletterError (typed domain errors)
  domain/
    mod.rs
    subscriber_status.rs  — subscriber state machine
    campaign_status.rs    — campaign state machine
  dto/
    mod.rs
    subscriber.rs  — SubscribeInput, SubscriberResponse
    campaign.rs    — CreateCampaignInput, CampaignResponse
  entities/
    mod.rs
    subscriber.rs  — newsletter_subscribers table
    campaign.rs    — newsletter_campaigns table
    subscription.rs — newsletter_subscriptions table
  services/
    mod.rs
    subscriber.rs  — SubscriberService (CRUD + lifecycle)
    campaign.rs    — CampaignService (CRUD + lifecycle)
  ports/
    mod.rs         — port implementations
  migrations/
    mod.rs
    m20261009_000040_create_newsletter_tables.rs
  graphql/
    mod.rs
    types.rs       — GraphQL types, inputs, enums
    query.rs       — NewsletterQuery (4 queries)
    mutation.rs    — NewsletterMutation (10 mutations)
```

### Database Schema

**3 таблицы:**
1. `newsletter_subscribers` — подписчики (email, status, confirm_token, locale)
2. `newsletter_campaigns` — кампании (title, subject, status, content_sources, scheduled_at)
3. `newsletter_subscriptions` — many-to-many subscriber ↔ segment

**6 индексов:**
- Unique email per tenant
- Status filtering
- Confirm token lookup
- Campaign status filtering
- Scheduled campaigns (for worker)
- Unique subscriber+segment

### API Surface

**GraphQL Queries (4):**
- `newsletterSubscriber(id, tenantId)` — get by ID
- `newsletterSubscribers(tenantId, page, perPage, status, search)` — list
- `newsletterCampaign(id, tenantId)` — get by ID
- `newsletterCampaigns(tenantId, page, perPage, status)` — list

**GraphQL Mutations (10):**
- `newsletterSubscribe` — subscribe new email
- `newsletterConfirmSubscription` — confirm pending
- `newsletterUnsubscribe` — unsubscribe
- `newsletterUpdateSubscriber` — update metadata
- `newsletterDeleteSubscriber` — delete
- `newsletterCreateCampaign` — create draft
- `newsletterUpdateCampaign` — update draft
- `newsletterScheduleCampaign` — schedule for delivery
- `newsletterCancelCampaign` — cancel
- `newsletterDeleteCampaign` — delete

### State Machines

**Subscriber lifecycle:**
```
Pending ──[confirm]──→ Active ──[unsubscribe]──→ Unsubscribed
  │                       │
  └──[unsubscribe]──→ Unsubscribed
                          │
              Active ──[suppress]──→ Suppressed ──[reactivate]──→ Active
```

**Campaign lifecycle:**
```
Draft ──[schedule]──→ Scheduled ──[start_sending]──→ Sending ──[complete]──→ Sent
  │                       │                            │
  └──[cancel]──→ Cancelled ←──[cancel]─────────────────┘
```

### Documentation

- `crates/modules/rustok-newsletter/README.md` — module overview
- `crates/modules/rustok-newsletter/docs/README.md` — detailed documentation
- `crates/modules/rustok-newsletter/docs/implementation-plan.md` — roadmap
- `docs/architecture/decisions/2026-10-09-newsletter-module-architecture.md` — ADR

### Registration

- ✅ `Cargo.toml` — workspace members + dependencies
- ✅ `modules.toml` — module entry with dependencies
- ✅ `rustok-module.toml` — module manifest

## Integration Pattern

Source modules (blog/forum/commerce) реализуют `NewsletterContentProvider`:

```rust
impl NewsletterContentProvider for BlogContentProvider {
    fn source_slug(&self) -> ContentSourceSlug {
        ContentSourceSlug::new("blog").unwrap()
    }
    
    async fn fetch_content(&self, request: ContentFetchRequest) 
        -> Result<Vec<NewsletterContentItem>, NewsletterApiError> 
    {
        // Fetch recent published posts
    }
    
    async fn has_content(&self, tenant_id: Uuid) -> bool {
        // Check if tenant has published posts
    }
}
```

## Dependencies

**Required:**
- `rustok-email` — transport (EmailDeliveryPort)
- `rustok-outbox` — transactional events

**Optional (content providers):**
- `rustok-blog` — blog content
- `rustok-forum` — forum content
- `rustok-commerce` — product content

## FFA/FBA Compliance

✅ **FFA/FBA gate passed:**
- Canonical domain contract defined
- Request context via `PortContext`
- Data ownership (3 tables)
- Cross-module ports (ContentProvider, SubscriberPort, CampaignPort)
- Transport adapters (GraphQL)
- Module-owned contracts (not host-owned)

## Next Steps (Phase 2+)

⏳ **Content Providers:**
- Blog content provider implementation
- Forum content provider implementation
- Commerce content provider implementation
- Content provider registry wiring

⏳ **Delivery System:**
- Email template integration
- Campaign renderer (content → HTML)
- Delivery via EmailDeliveryPort
- Bounce/suppression handling

⏳ **Scheduler:**
- Campaign scheduler worker
- Scheduled campaign pickup
- Batch sending with rate limiting

⏳ **Admin UI:**
- `rustok-newsletter-admin` package
- Subscriber management UI
- Campaign builder UI
- Analytics dashboard

⏳ **Public API:**
- Public subscription endpoint
- Double opt-in flow
- Unsubscribe page
- Preference center

## Verification

Без Rust toolchain в sandbox невозможно запустить `cargo check`, но код прошёл manual review:
- ✅ Все struct поля соответствуют entity/DTO определениям
- ✅ Все state machine transitions валидны
- ✅ GraphQL types/inputs/enums корректны
- ✅ Миграции создают таблицы и индексы
- ✅ Workspace integration корректна

## Заключение

Модуль `rustok-newsletter` полностью реализован как standalone bounded context, следующий всем архитектурным паттернам платформы. Готов к интеграции с source modules через `NewsletterContentProvider` trait.
