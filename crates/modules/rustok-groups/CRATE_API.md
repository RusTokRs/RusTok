# rustok-groups / CRATE_API

## Public surface

- Modules: `applications`, `domain`, `dto`, `entities`, `error`, `governance`, `invitations`, `localization`, `membership_enforcement`, `migrations`, `module`, `policy_history`, `ports`, `targeted_invitations`.
- Types: `GroupsModule`, `GroupsService`, `GroupMembershipEnforcementService`, `GroupMembershipEnforcementCommandService`, `GroupLocalizationService`, `GroupInvitationService`, `GroupTargetedInvitationService`, `GroupApplicationService`, `GroupApplicationPolicyHistoryService`, `GroupGovernanceService`, `GroupsError`, `GroupsResult<T>`.
- Core DTOs: `CreateGroupInput`, `GroupSummary`, `GroupDetails`, `GroupMembership`, `GroupMembershipEffectiveState`, `GroupAccessDecision`, `ChangeGroupRoleRequest`, `TransferGroupOwnershipRequest`, `SuspendGroupMembershipRequest`.

## Public modules

`applications`, `domain`, `dto`, `entities`, `error`, `governance`, `invitations`, `localization`, `membership_enforcement`, `migrations`, `module`, `policy_history`, `ports`, `targeted_invitations`.

## Platform Capability Bindings

- **Core Module Registration**: Exposes `GroupsModule` implementing `RusToKModule` (slug `"groups"`) and `MigrationSource`.
- **Primary Group Services & Facades**:
  - `GroupsService`: Effective membership and group presentation boundary.
  - `GroupMembershipEnforcementService` & `GroupMembershipEnforcementCommandService`: Single-membership enforcement and suspension management.
  - `GroupLocalizationService`: Multilingual translation lifecycle for group metadata.
  - `GroupInvitationService` & `GroupTargetedInvitationService`: Secure token and targeted group invitations.
  - `GroupApplicationService` & `GroupApplicationPolicyHistoryService`: Application submission, policy CAS, and review workflows.
  - `GroupGovernanceService`: Local role management and ownership transfers.
- **Provider & Capability Contracts**:
  - `GroupCapabilityDescriptor`: Declares supported ports, access fallback (`deny`), and protocol version.
  - Registers `GroupsModerationSubjectAdapterFactory` for moderation subsystem integration.
  - Registers `GroupsNotificationSourceProviderFactory` for notification dispatch.

## Domain Collaborations

- **`rustok-moderation`**: Implements `ModerationSubjectCommandPort` for group membership sanctions and suspensions.
- **`rustok-notifications`**: Dispatches invitation and membership events.
- **`rustok-outbox` / `rustok-events`**: Emits domain events (`group.created`, `group.membership.suspended`, etc.) through transactional outbox.
- **`rustok-media`**: Associates group avatar and cover images with tenant asset isolation.

## Minimum Contract Set

### Input DTOs/Commands

- `CreateGroupInput`, `ReadGroupRequest`, `ListGroupsRequest`, `JoinGroupRequest`, `LeaveGroupRequest`.
- `ChangeGroupRoleRequest`, `TransferGroupOwnershipRequest`.
- `SuspendGroupMembershipRequest`, `RevokeGroupMembershipSuspensionRequest`.
- `CreateGroupInvitationRequest`, `AcceptGroupInvitationRequest`.
- `SubmitGroupMembershipApplicationRequest`, `ReviewGroupMembershipApplicationRequest`.

### Domain Invariants

- Tenant isolation: All queries, mutations, and policy evaluations are strictly tenant-scoped.
- Group handles: Normalized ASCII lowercase, unique per tenant, bounded between 3 and 80 characters.
- Ownership protection: An active group must have exactly one owner; owners cannot change their own role without ownership transfer.
- Monotonic membership revisions: Membership changes advance monotonic revision counters to detect concurrent mutations.
- Effective access gating: Suspended or banned members fail closed for private/closed group interactions.

### Events / Outbox Side Effects

- Appends transactional domain events: group creation, membership activation, role change, ownership transfer, and suspension/revocation.
- Generates immutable audit log entries for governance mutations.

### Errors / Failure Codes

- `GroupsError` variants: `Validation`, `NotFound`, `Conflict`, `Forbidden`, `Database`.
- Standard port errors: `groups.invalid_handle`, `groups.invalid_title`, `groups.application_policy_changed`, etc.
