from pathlib import Path

path = Path("scripts/agent/apply_blog_richtext_storage_cutover.py")
source = path.read_text(encoding="utf-8")

def replace_once(label: str, old: str, new: str) -> None:
    global source
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected once, got {count}")
    source = source.replace(old, new, 1)

replace_once("owner create",
'once(svc, "            body,\\n            body_format,\\n            content_json,\\n            content,\\n", "            content,\\n")',
'once(svc, "        let CreatePostInput {\\n            locale,\\n            title,\\n            body,\\n            body_format,\\n            content_json,\\n            content,\\n", "        let CreatePostInput {\\n            locale,\\n            title,\\n            content,\\n")')

replace_once("graphql create",
"once(gql, '''    pub body: Option<String>,\n    pub body_format: Option<String>,\n    pub content_json: Option<Value>,\n    pub content: Option<RichTextDocument>,\n''', '''    pub content: RichTextDocument,\n''')",
"once(gql, '''#[derive(InputObject)]\npub struct CreatePostInput {\n    pub locale: String,\n    pub title: String,\n    pub body: Option<String>,\n    pub body_format: Option<String>,\n    pub content_json: Option<Value>,\n    pub content: Option<RichTextDocument>,\n''', '''#[derive(InputObject)]\npub struct CreatePostInput {\n    pub locale: String,\n    pub title: String,\n    pub content: RichTextDocument,\n''')")

replace_once("graphql update",
"once(gql, '''    pub body: Option<String>,\n    pub body_format: Option<String>,\n    pub content_json: Option<Value>,\n    pub content: Option<RichTextDocument>,\n''', '''    pub content: Option<RichTextDocument>,\n''')",
"once(gql, '''#[derive(InputObject)]\npub struct UpdatePostInput {\n    pub locale: Option<String>,\n    pub title: Option<String>,\n    pub body: Option<String>,\n    pub body_format: Option<String>,\n    pub content_json: Option<Value>,\n    pub content: Option<RichTextDocument>,\n''', '''#[derive(InputObject)]\npub struct UpdatePostInput {\n    pub locale: Option<String>,\n    pub title: Option<String>,\n    pub content: Option<RichTextDocument>,\n''')")

replace_once("orchestration Blog storage",
"once(orch, '''            body: Set(translation.body.clone()),\n            body_format: Set(translation.body_format.clone()),\n            created_at: Set(translation.created_at),\n''', '''            body: Set(translation.body.clone()),\n            created_at: Set(translation.created_at),\n''')",
"once(orch, '''            seo_description: Set(None),\n            body: Set(translation.body.clone()),\n            body_format: Set(translation.body_format.clone()),\n            created_at: Set(translation.created_at),\n''', '''            seo_description: Set(None),\n            body: Set(translation.body.clone()),\n            created_at: Set(translation.created_at),\n''')")

replace_once("dto legacy-key test",
'        assert!(encoded.get("body").is_none());\n        assert!(encoded.get("body_format").is_none());\n        assert!(encoded.get("content_json").is_none());\n',
"")

replace_once("mechanical service inclusion",
'    "crates/rustok-blog/src/graphql/types.rs",\n    "crates/rustok-blog/src/services/post.rs",\n    "crates/rustok-blog/src/entities/blog_post_translation.rs",\n',
'    "crates/rustok-blog/src/graphql/types.rs",\n    "crates/rustok-blog/src/entities/blog_post_translation.rs",\n')

replace_once("mechanical canonicalization order",
'    # Existing canonical expression wins.\n    s = re.sub(\n        r\'(?m)^(?P<i>\\s*)body:\\s*[^\\n]+,\\n(?P=i)body_format:\\s*[^\\n]+,\\n(?P=i)content_json:\\s*[^\\n]+,\\n(?P=i)content:\\s*(?P<c>[^\\n]+),$\',\n        lambda m: f"{m.group(\'i\')}content: {m.group(\'c\')},", s)\n    # Plain create input.\n    s = re.sub(\n        r\'(?m)^(?P<i>\\s*)body:\\s*(?P<b>[^\\n]+),\\n(?P=i)body_format:\\s*[^\\n]+,\\n(?P=i)content_json:\\s*[^\\n]+,\\n(?P=i)content:\\s*None,$\',\n        lambda m: f"{m.group(\'i\')}content: rustok_blog::richtext::article_document_from_plain_text(&{m.group(\'b\')}),", s)\n    # Plain update input.\n    s = re.sub(\n        r\'(?m)^(?P<i>\\s*)body:\\s*Some\\((?P<b>[^\\n]+)\\),\\n(?P=i)body_format:\\s*[^\\n]+,\\n(?P=i)content_json:\\s*[^\\n]+,\\n(?P=i)content:\\s*None,$\',\n        lambda m: f"{m.group(\'i\')}content: Some(rustok_blog::richtext::article_document_from_plain_text(&{m.group(\'b\')})),", s)\n',
'    # Plain update input must win before the general four-field collapse.\n    s = re.sub(\n        r\'(?m)^(?P<i>\\s*)body:\\s*Some\\((?P<b>[^\\n]+)\\),\\n(?P=i)body_format:\\s*[^\\n]+,\\n(?P=i)content_json:\\s*[^\\n]+,\\n(?P=i)content:\\s*None,$\',\n        lambda m: f"{m.group(\'i\')}content: Some(rustok_blog::richtext::article_document_from_plain_text(&{m.group(\'b\')})),", s)\n    # An untouched update keeps optional content absent.\n    s = re.sub(\n        r\'(?m)^(?P<i>\\s*)body:\\s*None,\\n(?P=i)body_format:\\s*None,\\n(?P=i)content_json:\\s*None,\\n(?P=i)content:\\s*None,$\',\n        lambda m: f"{m.group(\'i\')}content: None,", s)\n    # Plain create input becomes a canonical owner document.\n    s = re.sub(\n        r\'(?m)^(?P<i>\\s*)body:\\s*(?P<b>[^\\n]+),\\n(?P=i)body_format:\\s*[^\\n]+,\\n(?P=i)content_json:\\s*[^\\n]+,\\n(?P=i)content:\\s*None,$\',\n        lambda m: f"{m.group(\'i\')}content: rustok_blog::richtext::article_document_from_plain_text(&{m.group(\'b\')}),", s)\n    # Existing canonical expression wins.\n    s = re.sub(\n        r\'(?m)^(?P<i>\\s*)body:\\s*[^\\n]+,\\n(?P=i)body_format:\\s*[^\\n]+,\\n(?P=i)content_json:\\s*[^\\n]+,\\n(?P=i)content:\\s*(?P<c>[^\\n]+),$\',\n        lambda m: f"{m.group(\'i\')}content: {m.group(\'c\')},", s)\n')

replace_once("storefront helper test cleanup insertion",
'# Orchestration cannot bypass Blog owner storage.\n',
'# Remove tests for physically deleted storefront compatibility helpers.\nrx(\n    core,\n    r"\\n    #\\[test\\]\\n    fn summarize_content_handles_markdown_and_raw\\(\\) \\{.*?\\n    \\}\\n",\n    "\\n",\n    re.S,\n)\nrx(\n    core,\n    r"\\n    #\\[test\\]\\n    fn summarized_body_or_fallback_handles_none_and_raw_payload\\(\\) \\{.*?\\n    \\}\\n",\n    "\\n",\n    re.S,\n)\nonce(\n    core,\n    \'\'\'        assert_eq!(\n            body_or_fallback(None, "No body content yet."),\n            "No body content yet.".to_string()\n        );\n\'\'\',\n    "",\n)\n\n# Orchestration cannot bypass Blog owner storage.\n')

path.write_text(source, encoding="utf-8")
