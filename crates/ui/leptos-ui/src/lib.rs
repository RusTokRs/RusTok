/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

// Re-export core FFA components, types, and utilities from rustok-ui-leptos
pub use rustok_ui_leptos::*;

// Local composite components
pub mod language_toggle;
pub mod richtext;
pub mod success_message;
pub mod toc;

pub use language_toggle::{LanguageToggle as ui_language_toggle, LanguageToggleOption};
pub use richtext::{
    RichTextEditorFrame, RichTextFrameCopy, RichTextHtml, localized_richtext_frame_copy,
};
pub use success_message::SuccessMessage as ui_success_message;
pub use toc::{
    TableOfContents, TableOfContents as ui_table_of_contents, TocItem, extract_headings_from_html,
};
