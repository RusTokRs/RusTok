# rustok-messenger / CRATE_API

This inventory describes source entry points. Release readiness, current gaps and proposed contract changes are tracked in the [implementation plan](docs/implementation-plan.md).

## Public surface

### Module Facade
- `MessengerModule`
- `MessengerService`
- `MessengerPort`
- `MessengerPortAdapter`
- `MessengerError`
- `MessengerResult`

### Core Domain & DTOs
- `Conversation`, `ConversationType`
- `Message`, `MessageType`
- `Member`, `MemberRole`
- `SyncCursor`, `CursorOutput`
- `SnowflakeGenerator`
- `CreateConversationInput`, `ConversationOutput`
- `SendMessageInput`, `EditMessageInput`, `MessageOutput`
- `ListMessagesQuery`, `SearchMessagesQuery`
- `AddMemberInput`, `MemberOutput`
- `FastSyncRequest`, `FastSyncResponse`
- `UpdateCursorInput`
- `UpdateChannelTranslationInput`, `ChannelTranslationOutput`

### End-to-End Encryption (E2EE)
- `DeviceKeyRegistry`
- `DeviceKey`, `OneTimePrekey`, `PrekeyBundle`
- `UploadDeviceKeysInput`, `OneTimePrekeyInput`
- `EncryptedPayload`, `E2eeEncryptionAlgo`

### Streaming & Federation
- `MessengerStreamEvent`
- `DEFAULT_MESSENGER_PARTITIONS`, `MESSENGER_CONSUMER_GROUP`, `MESSENGER_STREAM_TOPIC`
- `messenger_calculate_partition`, `messenger_partition_key`
- `MatrixEventBridge`, `MatrixGatewayAdapter`, `MatrixRoomBridge`, `InboundMatrixMessagePayload`

### Notification Integration
- `MessengerNotificationSourceProvider`
- `MessengerNotificationSourceProviderFactory`
- `MESSENGER_SOURCE`, `MESSAGE_DISPATCHED_TYPE`, `MESSENGER_CONVERSATION_TARGET`
- `messenger_source_slug`, `message_dispatched_type`, `messenger_conversation_target_kind`

### Transports & GraphQL
- `axum_router`, `router`, `MessengerHttpRuntime`
- `MessengerClientMessage`, `MessengerServerMessage`, `MessengerRoomHub`
- `MessengerQuery`, `MessengerMutation` (feature = `graphql`)
- `attach_schema_data`, `MessengerGraphqlRuntimeData` (feature = `graphql`)
- `CreateMessengerChannelGqlInput`, `DeviceKeyGql`, `MessengerChannelListItemGql` (feature = `graphql`)
- `MessengerE2eeDeviceAuditGql`, `MessengerOverviewGql`, `OneTimePrekeyGql` (feature = `graphql`)
- `PrekeyBundleGql`, `UploadDeviceKeysGqlInput` (feature = `graphql`)

## Platform Capability Bindings
- **Core Module Registration**: Exposes `MessengerModule` implementing `RusToKModule` (slug `"messenger"`, kind `ModuleKind::Optional`, dependencies `["outbox"]`) and `MigrationSource`.
- **Messaging Services**:
  - `MessengerService`: Application service managing conversations, messages, memberships, cursors, translations, and search.
  - `MessengerPort` / `MessengerPortAdapter`: Cross-module port for system and direct messaging.
- **E2EE Infrastructure**:
  - `DeviceKeyRegistry`: In-memory thread-safe key registry for public device identities and one-time prekeys.
- **Notification Integration**:
  - `MessengerNotificationSourceProvider`: Multi-channel notification candidate materialization with zero plaintext leakage for encrypted conversations.
  - `MessengerNotificationSourceProviderFactory`: Provider factory registered into platform extensions.

## Domain Collaborations
- **Core Platform**: Integrates with `rustok-core` module system and `rustok-api` port context, permissions, and actor identities.
- **`rustok-outbox`**: Publishes `MessengerMessageDispatched` domain events inside transactions.
- **`rustok-notifications`**: Dispatches notification candidates for room participants excluding senders and muted members.

## Minimum Contract Set

### Input DTOs/Commands
- `CreateConversationInput` (`conversation_type`, `title`, `entity_type`, `entity_id`, `participant_ids`, `is_encrypted`, `initial_locale`, `initial_channel_description`)
- `SendMessageInput` (`conversation_id`, `message_type`, `content`, `blocks_json`, `reply_to_id`, `is_encrypted`, `encryption_algo`)
- `EditMessageInput` (`content`)
- `AddMemberInput` (`user_id`, `role`)
- `UpdateCursorInput` (`conversation_id`, `last_read_message_id`)
- `UpdateChannelTranslationInput` (`channel_id`, `locale`, `title`, `description`)
- `FastSyncRequest` (`conversation_id`, `since_seq`, `limit`)
- `ListMessagesQuery` (`conversation_id`, `limit`, `before_id`, `after_id`)
- `SearchMessagesQuery` (`query`, `conversation_id`, `limit`, `offset`)

### Current validation and limits

- Message IDs come from per-service Snowflake generators. History and FastSync use message IDs as positions. An in-place edit or delete does not create a new position.
- Cursor progression rejects a lower value in a single update. The current write lacks atomic monotonicity and tenant-scoped room admission.
- Tenant and membership checks vary by operation. Consult the plan's source baseline and privacy gates before releasing the existing entry points.
- Search query length is limited to 500 characters, message content to 64 KiB, channel title to 255 characters, and locale code to 32 characters.
- The device registry and encrypted envelopes supply building blocks. A supported E2EE client protocol and durable tenant-scoped key storage remain separate work.

### Events / Outbox Side Effects
- `MessengerMessageDispatched` published to transactional outbox on every successful message creation.
- Realtime adapter calls publish to a process-local WebSocket hub. The optional Iggy dispatcher in this crate does not send a broker message.

### Errors / Failure Codes
- `MessengerError::ConversationNotFound` — conversation does not exist.
- `MessengerError::ConversationAccessDenied` — actor is not a member or lacks permissions.
- `MessengerError::MemberNotFound` / `MemberAlreadyExists` — membership conflicts.
- `MessengerError::MessageNotFound` — target message ID does not exist.
- `MessengerError::InvalidSequence` / `CursorRegression` — cursor monotonicity violations.
- `MessengerError::PayloadTooLarge` — message size exceeds 64 KiB limit.
- `MessengerError::Validation` — nil UUIDs, invalid bounds, or unsupported algorithms.
- `MessengerError::Database` — SeaORM connection or execution failure.
