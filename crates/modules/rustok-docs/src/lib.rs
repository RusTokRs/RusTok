//! # rustok-docs
//!
//! Sovereign collaborative documents, wiki, and knowledge base module powered by Loro CRDT.
//!
//! Provides real-time multiplayer editing, hierarchical MovableTree block structures,
//! rich-text delta logs, snapshots, and offline-first synchronization.

pub mod controllers;
pub mod domain;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod services;

pub use controllers::{
    ClientMessage, DocsHttpRuntime, DocsRoomHub, ServerMessage, axum_router, router,
};
pub use domain::{
    BlockType, CollaborativeDoc, DatabaseViewType, DocBlock, EntityDisplayMode, EntityRef,
};
pub use dto::{
    ApplyDeltaDto, CreatePageDto, DocUpdateItemDto, DocumentSyncDto, PageSummaryDto,
    PageTreeNodeDto, UpdatePageDto,
};
pub use error::{DocsError, DocsResult};
pub use module::DocsModule;
pub use services::{COMPACTION_THRESHOLD, DocumentService};
pub use loro::{Frontiers, LoroDoc, TreeID, VersionVector};

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_create_blocks_and_render_markdown() {
        let doc = CollaborativeDoc::new();

        let h1 = doc.create_block(None, BlockType::Heading1, "RusToK Wiki Architecture").unwrap();
        let p1 = doc.create_block(None, BlockType::Paragraph, "Collaborative documents powered by Loro CRDT.").unwrap();

        let list1 = doc.create_block(Some(p1), BlockType::BulletList, "Zero merge conflicts").unwrap();
        let _list2 = doc.create_block(Some(p1), BlockType::BulletList, "Real-time sync").unwrap();

        assert_eq!(doc.get_block_text(h1), "RusToK Wiki Architecture");
        assert_eq!(doc.get_block_text(list1), "Zero merge conflicts");

        let md = doc.render_markdown();
        assert!(md.contains("# RusToK Wiki Architecture"));
        assert!(md.contains("Collaborative documents powered by Loro CRDT."));
        assert!(md.contains("* Zero merge conflicts"));

        let blocks = doc.get_blocks();
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].content, "RusToK Wiki Architecture");
    }

    #[test]
    fn test_concurrent_multiplayer_editing_sync() {
        // Peer 1 (Alice)
        let alice_doc = CollaborativeDoc::with_peer_id(1);
        let block_id = alice_doc.create_block(None, BlockType::Paragraph, "Hello").unwrap();

        // Snapshot sent to Peer 2 (Bob)
        let snapshot = alice_doc.export_snapshot().unwrap();
        let bob_doc = CollaborativeDoc::with_peer_id(2);
        bob_doc.import_updates(&snapshot).unwrap();

        assert_eq!(bob_doc.get_block_text(block_id), "Hello");

        // Alice edits: appends " World"
        alice_doc.insert_block_text(block_id, 5, " World").unwrap();

        // Bob concurrently edits: prepends "Greetings! "
        bob_doc.insert_block_text(block_id, 0, "Greetings! ").unwrap();

        // Exchange updates between Alice and Bob
        let alice_delta = alice_doc.export_all_updates().unwrap();
        let bob_delta = bob_doc.export_all_updates().unwrap();

        bob_doc.import_updates(&alice_delta).unwrap();
        alice_doc.import_updates(&bob_delta).unwrap();

        // Both peers converge deterministically to the identical text!
        let alice_text = alice_doc.get_block_text(block_id);
        let bob_text = bob_doc.get_block_text(block_id);

        assert_eq!(alice_text, bob_text, "CRDT must converge to identical state");
        assert!(alice_text.contains("Hello"));
        assert!(alice_text.contains("World"));
        assert!(alice_text.contains("Greetings!"));
    }

    #[test]
    fn test_movable_tree_drag_and_drop_reorder() {
        let doc = CollaborativeDoc::new();

        let root1 = doc.create_block(None, BlockType::Heading2, "Section 1").unwrap();
        let root2 = doc.create_block(None, BlockType::Heading2, "Section 2").unwrap();
        let item = doc.create_block(Some(root1), BlockType::Paragraph, "Item under section 1").unwrap();

        assert_eq!(doc.get_block_text(item), "Item under section 1");

        // Drag and drop: move item under root2
        doc.move_block(item, Some(root2), 0).unwrap();

        let md = doc.render_markdown();
        assert!(md.contains("## Section 1"));
        assert!(md.contains("## Section 2"));

        let blocks = doc.get_blocks();
        let item_block = blocks.iter().find(|b| b.content == "Item under section 1").unwrap();
        assert_eq!(item_block.parent_id, Some(root2.to_string()));
    }

    #[test]
    fn test_entity_embed_and_block_mutation() {
        let doc = CollaborativeDoc::new();

        let tenant_id = Uuid::new_v4();
        let product_id = Uuid::new_v4();
        let entity_ref = EntityRef {
            tenant_id,
            module: "commerce".to_string(),
            entity_type: "product".to_string(),
            entity_id: product_id,
            display_mode: EntityDisplayMode::CompactCard,
        };

        let block = doc.create_block(None, BlockType::EntityEmbed(entity_ref.clone()), "").unwrap();

        // Update block properties with custom styling
        doc.update_block_properties(block, serde_json::json!({ "theme": "dark", "badge": "New" })).unwrap();

        let blocks = doc.get_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::EntityEmbed(entity_ref));
        assert_eq!(blocks[0].properties["theme"], "dark");

        let md = doc.render_markdown();
        assert!(md.contains("entity-embed"));
    }
}
