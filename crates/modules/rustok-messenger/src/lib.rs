//! # RusToK Messenger Module
//!
//! `rustok-messenger` provides highload distributed team messaging, channels, direct
//! messages, monotonic sequence cursors, and cryptographic foundations for E2EE.

pub mod controllers;
pub mod domain;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;
pub mod streaming;

pub use streaming::{
    DEFAULT_MESSENGER_PARTITIONS, MESSENGER_CONSUMER_GROUP, MESSENGER_STREAM_TOPIC,
    MessengerStreamEvent, messenger_calculate_partition, messenger_partition_key,
};

pub use domain::{
    Conversation, ConversationType, Member, MemberRole, Message, MessageType, SnowflakeGenerator,
    SyncCursor,
};
pub use dto::{
    ChannelTranslationOutput, ConversationOutput, CreateConversationInput, CursorOutput,
    FastSyncRequest, FastSyncResponse, ListMessagesQuery, MessageOutput, SendMessageInput,
    UpdateChannelTranslationInput, UpdateCursorInput,
};
pub use controllers::{
    MessengerClientMessage, MessengerHttpRuntime, MessengerRoomHub, MessengerServerMessage, router,
};
pub use error::{MessengerError, MessengerResult};
pub use module::MessengerModule;
pub use ports::{MessengerPort, MessengerPortAdapter};
pub use services::MessengerService;
