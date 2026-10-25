use super::*;
use rustok_api::{RichTextDocument, RichTextView};
use rustok_comments::CommentRecord;

#[test]
fn map_comment_record_preserves_the_typed_projection() {
    let document: RichTextDocument = serde_json::from_value(serde_json::json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": "Hello"}]
        }]
    }))
    .expect("test richtext");
    let record = CommentRecord {
        id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        target_type: TARGET_TYPE_BLOG_POST.to_string(),
        target_id: Uuid::new_v4(),
        requested_locale: "en".into(),
        effective_locale: "en".into(),
        author_id: Uuid::new_v4(),
        parent_comment_id: None,
        body: RichTextView {
            document: document.clone(),
            html: "<p class=\"richtext-paragraph\">Hello</p>".into(),
        },
        body_text: "Hello".into(),
        status: ApiCommentStatus::Pending,
        position: 1,
        created_at: "2024-01-01T00:00:00Z".into(),
        updated_at: "2024-01-01T00:00:00Z".into(),
    };

    let response = CommentService::map_comment_record(record).expect("mapping should succeed");

    assert_eq!(response.content.document, document);
    assert_eq!(response.content_text, "Hello");
}
