# rustok-profiles / CRATE_API

## Public surface

- Modules: `api`, `dto`, `entities`, `error`, `graphql`, `loader`, `media`, `migrations`, `mutations`, `observability`, `presentation`, `privacy`, `reader`, `services`, `module`.
- Types: `ProfilesModule`, `ProfileService`, `ProfileMutationService`, `ProfilesReader`, `ProfilePrivacyService`, `ProfileSummaryLoader`, `ProfileError`, `ProfileResult<T>`.
- Core DTOs: `UpsertProfileInput`, `ProfileSummary`, `ProfileVisibility`, `ProfileStatus`, `ProfileAccessAudience`.

## Platform Capability Bindings
- **RusToKModule Implementation**: `ProfilesModule` in `src/module.rs`
- **Module Slug**: `profiles`
- **Module Name**: `Profiles`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `["media", "social_graph", "taxonomy"]`
- **Permissions**:
  - `PROFILES_CREATE`
  - `PROFILES_READ`
  - `PROFILES_UPDATE`
  - `PROFILES_DELETE`
  - `PROFILES_LIST`
  - `PROFILES_MANAGE`
- **Database Migrations**: Sea-ORM migrations defined in `src/migrations/mod.rs` with migration dependencies on `media`, `social_graph`, and `taxonomy`.

## Domain Collaborations
- **Media**: Validates and attaches user profile avatars and header banners (`ProfileMediaPublicImageProvider`). Enforces tenant isolation on assets.
- **Social Graph**: Queries follower/following counters and follower relationship statuses to enforce `FollowersOnly` profile visibility.
- **Taxonomy**: Profile interest tags, categorization, and discoverability metadata.
- **Outbox & Events**: Publishes `ProfileUpdatedEvent` into transactional outbox.
- **GraphQL**: Exposes `ProfilesQuery` and `ProfilesMutation` for public storefront profile presentation.

## Minimum Contract Set
- `ProfileService`: User profile fetching, handle normalization and reservation checks, presentation mapping.
- `ProfileMutationService`: Profile creation, update, visibility changes, and deactivation.
- `ProfilesReader` & `ProfileSummaryReader`: Cross-boundary read port for profile summaries.
- `ProfilePrivacyService`: Evaluates profile access rules (`evaluate_profile_access`) based on audience, owner status, blocks, and follower state.
- `ProfileSummaryLoader`: Batch dataloader for high-performance profile resolution across downstream modules.

### Input DTOs/Commands
- `UpsertProfileInput`: User ID, display name, handle, bio, avatar asset ID, banner asset ID, default locale, visibility (`Public`, `FollowersOnly`, `Private`).
- `ProfileBackfillRequest`: Batch backfill of missing profile records for existing accounts.
- `ProfileAccessAudience`: Security caller context (`Anonymous`, `Authenticated { viewer_id }`, `System`).

### Domain Invariants
- **Display Name Validity**: Must not be empty (`EmptyDisplayName`) and bounded in length (`DisplayNameTooLong`).
- **Handle Validation**: Must not be empty (`EmptyHandle`), valid ASCII alphanumeric/underscore characters (`InvalidHandle`), bounded by minimum/maximum length (`HandleTooShort`, `HandleTooLong`), not reserved (`ReservedHandle`), and globally unique (`DuplicateHandle`).
- **Locale Format**: Must be a valid BCP 47 language tag (`InvalidLocale`).
- **Privacy & Audience Gates**: Inactive, blocked, or hidden profiles fail closed before access (`ProfilePrivacyDecision`). `FollowersOnly` profiles require active follow relationships.
- **Tenant Asset Isolation**: Media assets used for avatar/banner must belong to the same tenant as the profile.

### Events / Outbox Side Effects
- `DomainEvent::ProfileUpdated`: Emitted upon profile creation, update, or deactivation via `TransactionalEventBus`.

### Errors / Failure Codes
- `ProfileError::EmptyDisplayName`: Display name is missing or empty.
- `ProfileError::DisplayNameTooLong`: Display name exceeds maximum character limit.
- `ProfileError::EmptyHandle`: Handle is empty.
- `ProfileError::InvalidHandle`: Handle contains disallowed characters.
- `ProfileError::HandleTooShort` / `HandleTooLong`: Handle length is outside permitted bounds.
- `ProfileError::ReservedHandle(String)`: Handle matches a reserved system or platform keyword.
- `ProfileError::InvalidLocale(String)`: Supplied locale tag is invalid.
- `ProfileError::ProfileNotFound(Uuid)`: Target profile does not exist by user ID.
- `ProfileError::ProfileByHandleNotFound(String)`: Target profile does not exist by handle.
- `ProfileError::DuplicateHandle(String)`: Handle collision with another user.
- `ProfileError::Validation(String)`: General invariant or input validation failure.
- `ProfileError::PresentationUnavailable`: Presentation formatting service failure.
- `ProfileError::EventPublishUnavailable`: Outbox event publishing failure.
- `ProfileError::TaxonomyUnavailable(String)`: Downstream taxonomy dependency unavailable.
- `ProfileError::Database(DbErr)`: Storage layer error.
