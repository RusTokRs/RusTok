from pathlib import Path

path = Path("crates/rustok-pages/src/services/page_builder_artifact.rs")
text = path.read_text()
unused = '''
    pub(crate) async fn clear_existing_body_binding_in_tx(
        txn: &DatabaseTransaction,
        page_id: Uuid,
        locale: &str,
    ) -> PagesResult<()> {
        let body = page_body::Entity::find()
            .filter(page_body::Column::PageId.eq(page_id))
            .filter(page_body::Column::Locale.eq(locale))
            .one(txn)
            .await?;
        if let Some(body) = body {
            page_published_landing_artifact::Entity::delete_by_id(body.id)
                .exec(txn)
                .await?;
        }
        Ok(())
    }
'''
if unused not in text:
    raise RuntimeError("unused body binding helper is missing")
path.write_text(text.replace(unused, "", 1))
