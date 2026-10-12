use std::sync::Arc;
use loro::{ExportMode, Frontiers, LoroDoc, TreeID, VersionVector};
use crate::error::{DocsError, DocsResult};
use super::block::{BlockType, DocBlock};

/// Collaborative document engine powered by Loro CRDT.
///
/// Encapsulates real-time multiplayer editing, hierarchical MovableTree blocks,
/// rich-text delta logs, snapshots, and time-travel checkouts.
#[derive(Clone)]
pub struct CollaborativeDoc {
    doc: Arc<LoroDoc>,
}

impl CollaborativeDoc {
    /// Creates a fresh collaborative document with a random peer ID.
    pub fn new() -> Self {
        Self {
            doc: Arc::new(LoroDoc::new()),
        }
    }

    /// Creates a document with a specific peer ID (useful for deterministic tests and client sessions).
    pub fn with_peer_id(peer_id: u64) -> Self {
        let doc = LoroDoc::new();
        doc.set_peer_id(peer_id).ok();
        Self { doc: Arc::new(doc) }
    }

    /// Restores a document from a raw binary snapshot or update log.
    pub fn from_snapshot(bytes: &[u8]) -> DocsResult<Self> {
        let doc = LoroDoc::new();
        doc.import(bytes).map_err(|e| DocsError::Loro(e.to_string()))?;
        Ok(Self { doc: Arc::new(doc) })
    }

    /// Returns the underlying LoroDoc reference.
    pub fn inner(&self) -> &LoroDoc {
        &self.doc
    }

    /// Appends a new block to the document tree.
    pub fn create_block(
        &self,
        parent: Option<TreeID>,
        block_type: BlockType,
        initial_content: &str,
    ) -> DocsResult<TreeID> {
        let tree = self.doc.get_tree("blocks");
        let node_id = tree.create(parent).map_err(|e| DocsError::Loro(e.to_string()))?;

        // Initialize block properties
        let meta = self.doc.get_map(format!("block_meta_{}", node_id));
        let type_str = serde_json::to_string(&block_type).map_err(DocsError::from)?;
        meta.insert("type", type_str.as_str()).map_err(|e| DocsError::Loro(e.to_string()))?;
        meta.insert("props", "{}").map_err(|e| DocsError::Loro(e.to_string()))?;

        // Initialize block text content
        if !initial_content.is_empty() {
            let text = self.doc.get_text(format!("block_text_{}", node_id));
            text.insert(0, initial_content).map_err(|e| DocsError::Loro(e.to_string()))?;
        }

        self.doc.commit();
        Ok(node_id)
    }

    /// Moves a block to a new parent or index in the tree (zero merge conflict / cycle-safe).
    pub fn move_block(
        &self,
        target: TreeID,
        new_parent: Option<TreeID>,
        index: usize,
    ) -> DocsResult<()> {
        let tree = self.doc.get_tree("blocks");
        tree.mov_to(target, new_parent, index).map_err(|e| DocsError::Loro(e.to_string()))?;
        self.doc.commit();
        Ok(())
    }

    /// Deletes a block and its subtree.
    pub fn delete_block(&self, target: TreeID) -> DocsResult<()> {
        let tree = self.doc.get_tree("blocks");
        tree.delete(target).map_err(|e| DocsError::Loro(e.to_string()))?;
        self.doc.commit();
        Ok(())
    }

    /// Updates the text content of an individual block.
    pub fn set_block_text(&self, node_id: TreeID, new_content: &str) -> DocsResult<()> {
        let text = self.doc.get_text(format!("block_text_{}", node_id));
        let current_len = text.len_unicode();
        if current_len > 0 {
            text.delete(0, current_len).map_err(|e| DocsError::Loro(e.to_string()))?;
        }
        if !new_content.is_empty() {
            text.insert(0, new_content).map_err(|e| DocsError::Loro(e.to_string()))?;
        }
        self.doc.commit();
        Ok(())
    }

    /// Appends or inserts text to an existing block (CRDT rich-text delta).
    pub fn insert_block_text(&self, node_id: TreeID, pos: usize, text_to_insert: &str) -> DocsResult<()> {
        let text = self.doc.get_text(format!("block_text_{}", node_id));
        text.insert(pos, text_to_insert).map_err(|e| DocsError::Loro(e.to_string()))?;
        self.doc.commit();
        Ok(())
    }

    /// Reads the current text of a block.
    pub fn get_block_text(&self, node_id: TreeID) -> String {
        let text = self.doc.get_text(format!("block_text_{}", node_id));
        text.to_string()
    }

    /// Exports the full document snapshot in compact Loro binary format.
    pub fn export_snapshot(&self) -> DocsResult<Vec<u8>> {
        self.doc.export(ExportMode::Snapshot).map_err(|e| DocsError::Loro(e.to_string()))
    }

    /// Exports all updates in binary format.
    pub fn export_all_updates(&self) -> DocsResult<Vec<u8>> {
        self.doc.export(ExportMode::all_updates()).map_err(|e| DocsError::Loro(e.to_string()))
    }

    /// Exports updates created since a given VersionVector.
    pub fn export_updates_from(&self, vv: &VersionVector) -> DocsResult<Vec<u8>> {
        self.doc.export(ExportMode::updates(vv)).map_err(|e| DocsError::Loro(e.to_string()))
    }

    /// Returns the OpLog VersionVector of the document.
    pub fn oplog_vv(&self) -> VersionVector {
        self.doc.oplog_vv()
    }

    /// Merges updates from a remote peer or client.
    pub fn import_updates(&self, updates: &[u8]) -> DocsResult<()> {
        self.doc.import(updates).map_err(|e| DocsError::Loro(e.to_string()))?;
        Ok(())
    }

    /// Returns the current state version vector (Frontiers).
    pub fn frontiers(&self) -> Frontiers {
        self.doc.state_frontiers()
    }

    /// Checks out a historical state of the document (Time-Travel).
    pub fn checkout(&self, frontiers: &Frontiers) -> DocsResult<()> {
        self.doc.checkout(frontiers).map_err(|e| DocsError::Loro(e.to_string()))?;
        Ok(())
    }

    /// Checks out the latest state (HEAD).
    pub fn checkout_to_latest(&self) -> DocsResult<()> {
        self.doc.checkout_to_latest();
        Ok(())
    }

    /// Returns all blocks structured as a flat list with parent/child relationships for API/UI.
    pub fn get_blocks(&self) -> Vec<DocBlock> {
        let tree = self.doc.get_tree("blocks");
        let roots = tree.children(None).unwrap_or_default();
        let mut blocks = Vec::new();

        for root_id in roots {
            self.collect_block_dto(&mut blocks, root_id, None);
        }

        blocks
    }

    fn collect_block_dto(&self, out: &mut Vec<DocBlock>, node_id: TreeID, parent_id: Option<String>) {
        let text = self.get_block_text(node_id);
        let meta = self.doc.get_map(format!("block_meta_{}", node_id));
        let type_val = meta.get("type").and_then(|v| v.into_value().ok());

        let block_type: BlockType = type_val
            .and_then(|v| v.into_string().ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(BlockType::Paragraph);

        let tree = self.doc.get_tree("blocks");
        let child_ids = tree.children(Some(node_id)).unwrap_or_default();
        let children_str = child_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();

        out.push(DocBlock {
            id: node_id.to_string(),
            parent_id,
            block_type,
            content: text,
            properties: serde_json::json!({}),
            children: children_str,
        });

        for child_id in child_ids {
            self.collect_block_dto(out, child_id, Some(node_id.to_string()));
        }
    }

    /// Renders a pure Markdown projection of the collaborative document.
    pub fn render_markdown(&self) -> String {
        let tree = self.doc.get_tree("blocks");
        let roots = tree.children(None).unwrap_or_default();
        let mut output = String::new();

        for root_id in roots {
            self.render_block_markdown(&mut output, root_id, 0);
        }

        output
    }

    fn render_block_markdown(&self, out: &mut String, node_id: TreeID, indent_level: usize) {
        let text = self.get_block_text(node_id);
        let meta = self.doc.get_map(format!("block_meta_{}", node_id));
        let type_val = meta.get("type").and_then(|v| v.into_value().ok());

        let block_type: BlockType = type_val
            .and_then(|v| v.into_string().ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(BlockType::Paragraph);

        let indent = "  ".repeat(indent_level);

        match block_type {
            BlockType::Heading1 => {
                out.push_str(&format!("# {}\n\n", text));
            }
            BlockType::Heading2 => {
                out.push_str(&format!("## {}\n\n", text));
            }
            BlockType::Heading3 => {
                out.push_str(&format!("### {}\n\n", text));
            }
            BlockType::Paragraph => {
                out.push_str(&format!("{}{}\n\n", indent, text));
            }
            BlockType::BulletList => {
                out.push_str(&format!("{}* {}\n", indent, text));
            }
            BlockType::NumberedList => {
                out.push_str(&format!("{}1. {}\n", indent, text));
            }
            BlockType::TodoList { checked } => {
                let mark = if checked { "[x]" } else { "[ ]" };
                out.push_str(&format!("{}{} {}\n", indent, mark, text));
            }
            BlockType::Quote => {
                out.push_str(&format!("{}> {}\n\n", indent, text));
            }
            BlockType::Callout { icon } => {
                out.push_str(&format!("{}> {} {}\n\n", indent, icon, text));
            }
            BlockType::Code { language } => {
                out.push_str(&format!("```{}\n{}\n```\n\n", language, text));
            }
            BlockType::Divider => {
                out.push_str("---\n\n");
            }
            BlockType::Table => {
                out.push_str(&format!("{}[Table: {}]\n\n", indent, text));
            }
            BlockType::LiveErp { entity_type, entity_id } => {
                out.push_str(&format!("<!-- live-erp: type={} id={} -->\n\n", entity_type, entity_id));
            }
        }

        // Recursively render child blocks
        let tree = self.doc.get_tree("blocks");
        if let Some(children) = tree.children(Some(node_id)) {
            for child_id in children {
                self.render_block_markdown(out, child_id, indent_level + 1);
            }
        }
    }
}

impl Default for CollaborativeDoc {
    fn default() -> Self {
        Self::new()
    }
}
