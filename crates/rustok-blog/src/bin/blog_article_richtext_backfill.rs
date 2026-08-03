use std::{
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use rustok_api::RichTextDocument;
use rustok_blog::richtext::{
    article_document_from_plain_text, canonical_article_body, normalize_article,
};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use sea_orm_migration::prelude::SchemaManager;
use serde::Serialize;
use serde_json::Value as JsonValue;
use uuid::Uuid;

const TARGET_FORMAT: &str = "richtext";
const DEFAULT_BATCH_SIZE: u64 = 500;
const MAX_BATCH_SIZE: u64 = 10_000;

#[derive(Debug, Clone)]
struct Cli {
    tenant_id: Option<Uuid>,
    batch_size: u64,
    apply: bool,
    allow_markdown_plain_text: bool,
    report: Option<PathBuf>,
    help: bool,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
struct Metrics {
    scanned: u64,
    already_canonical: u64,
    planned_updates: u64,
    applied: u64,
    invalid: u64,
}

#[derive(Debug, Clone)]
struct LegacyRow {
    id: Uuid,
    post_id: Uuid,
    tenant_id: Option<Uuid>,
    locale: String,
    body: String,
    body_format: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct Cursor {
    updated_at: DateTime<Utc>,
    id: Uuid,
}

#[derive(Debug, Clone, Copy)]
enum ConversionKind {
    AlreadyCanonical,
    NormalizeRichtext,
    LegacyEnvelope,
    MarkdownPlainText,
}

impl ConversionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyCanonical => "already_canonical",
            Self::NormalizeRichtext => "normalize_richtext",
            Self::LegacyEnvelope => "legacy_envelope",
            Self::MarkdownPlainText => "markdown_plain_text",
        }
    }
}

#[derive(Debug, Clone)]
struct Conversion {
    body: String,
    kind: ConversionKind,
}

impl Conversion {
    fn needs_update(&self, row: &LegacyRow) -> bool {
        self.body != row.body || row.body_format != TARGET_FORMAT
    }
}

#[derive(Debug, Serialize)]
struct ReportRecord {
    translation_id: Uuid,
    post_id: Uuid,
    tenant_id: Option<Uuid>,
    locale: String,
    source_format: String,
    action: String,
    message: Option<String>,
}

struct ReportWriter {
    inner: Option<BufWriter<File>>,
}

impl ReportWriter {
    fn new(path: Option<&Path>) -> Result<Self> {
        let inner = match path {
            Some(path) => {
                if let Some(parent) = path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("failed to create report directory {}", parent.display())
                    })?;
                }
                Some(BufWriter::new(File::create(path).with_context(|| {
                    format!("failed to create report {}", path.display())
                })?))
            }
            None => None,
        };
        Ok(Self { inner })
    }

    fn disabled() -> Self {
        Self { inner: None }
    }

    fn write(&mut self, record: &ReportRecord) -> Result<()> {
        if let Some(writer) = self.inner.as_mut() {
            serde_json::to_writer(&mut *writer, record)?;
            writer.write_all(b"\n")?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if let Some(writer) = self.inner.as_mut() {
            writer.flush()?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = parse_args(env::args().skip(1))?;
    if cli.help {
        print_usage();
        return Ok(());
    }

    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let db = Database::connect(&database_url)
        .await
        .context("failed to connect to DATABASE_URL")?;

    ensure_pre_cutover_schema(&db).await?;

    let mut report = ReportWriter::new(cli.report.as_deref())?;
    let preflight = preflight_pass(&db, &cli, &mut report).await?;
    report.flush()?;
    print_metrics("preflight", &preflight);

    if preflight.invalid > 0 {
        bail!(
            "preflight found {} rows that cannot be converted safely; no rows were written",
            preflight.invalid
        );
    }

    if !cli.apply {
        println!(
            "dry-run complete; rerun with --apply only after reviewing the report and database backup"
        );
        return Ok(());
    }

    let applied = apply_pass(&db, &cli).await?;
    print_metrics("apply", &applied);

    let verification = preflight_pass(&db, &cli, &mut ReportWriter::disabled()).await?;
    print_metrics("post-apply", &verification);
    if verification.invalid > 0 || verification.planned_updates > 0 {
        bail!(
            "post-apply verification failed: invalid={} planned_updates={}",
            verification.invalid,
            verification.planned_updates
        );
    }

    println!(
        "backfill complete; run the Blog richtext cutover migration and then retain Search/browser evidence"
    );
    Ok(())
}

async fn ensure_pre_cutover_schema(db: &DatabaseConnection) -> Result<()> {
    let manager = SchemaManager::new(db);
    if !manager.has_table("blog_posts").await? {
        bail!("blog_posts table is missing");
    }
    if !manager.has_table("blog_post_translations").await? {
        bail!("blog_post_translations table is missing");
    }
    if !manager
        .has_column("blog_post_translations", "body_format")
        .await?
    {
        bail!(
            "blog_post_translations.body_format is already absent; the irreversible cutover migration has already executed"
        );
    }
    Ok(())
}

async fn preflight_pass(
    db: &DatabaseConnection,
    cli: &Cli,
    report: &mut ReportWriter,
) -> Result<Metrics> {
    let mut metrics = Metrics::default();
    let mut cursor = None;

    loop {
        let rows = fetch_batch(db, cli.tenant_id, cursor, cli.batch_size).await?;
        if rows.is_empty() {
            break;
        }
        cursor = rows.last().map(|row| Cursor {
            updated_at: row.updated_at.clone(),
            id: row.id,
        });

        for row in rows {
            metrics.scanned += 1;
            match convert_row(&row, cli.allow_markdown_plain_text) {
                Ok(conversion) => {
                    let needs_update = conversion.needs_update(&row);
                    if needs_update {
                        metrics.planned_updates += 1;
                    } else {
                        metrics.already_canonical += 1;
                    }
                    report.write(&ReportRecord {
                        translation_id: row.id,
                        post_id: row.post_id,
                        tenant_id: row.tenant_id,
                        locale: row.locale.clone(),
                        source_format: row.body_format.clone(),
                        action: conversion.kind.as_str().to_string(),
                        message: None,
                    })?;
                }
                Err(error) => {
                    metrics.invalid += 1;
                    eprintln!(
                        "[invalid] translation_id={} post_id={} tenant_id={:?} locale={} format={} error={:#}",
                        row.id, row.post_id, row.tenant_id, row.locale, row.body_format, error
                    );
                    report.write(&ReportRecord {
                        translation_id: row.id,
                        post_id: row.post_id,
                        tenant_id: row.tenant_id,
                        locale: row.locale,
                        source_format: row.body_format,
                        action: "invalid".to_string(),
                        message: Some(error.to_string()),
                    })?;
                }
            }
        }
    }

    Ok(metrics)
}

async fn apply_pass(db: &DatabaseConnection, cli: &Cli) -> Result<Metrics> {
    let mut metrics = Metrics::default();
    let mut cursor = None;

    loop {
        let rows = fetch_batch(db, cli.tenant_id, cursor, cli.batch_size).await?;
        if rows.is_empty() {
            break;
        }
        cursor = rows.last().map(|row| Cursor {
            updated_at: row.updated_at.clone(),
            id: row.id,
        });

        let mut updates = Vec::new();
        for row in rows {
            metrics.scanned += 1;
            let conversion =
                convert_row(&row, cli.allow_markdown_plain_text).with_context(|| {
                    format!(
                        "row {} changed after preflight and is no longer convertible",
                        row.id
                    )
                })?;
            if conversion.needs_update(&row) {
                metrics.planned_updates += 1;
                updates.push((row, conversion));
            } else {
                metrics.already_canonical += 1;
            }
        }

        if updates.is_empty() {
            continue;
        }

        let txn = db.begin().await?;
        for (row, conversion) in &updates {
            if !optimistic_update(&txn, row, &conversion.body).await? {
                let translation_id = row.id;
                txn.rollback().await?;
                bail!(
                    "optimistic update conflict for Blog translation {translation_id}; rerun dry-run before retrying"
                );
            }
        }
        txn.commit().await?;
        metrics.applied += updates.len() as u64;
    }

    Ok(metrics)
}

async fn fetch_batch<C>(
    db: &C,
    tenant_id: Option<Uuid>,
    cursor: Option<Cursor>,
    batch_size: u64,
) -> Result<Vec<LegacyRow>>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let mut sql = String::from(
        "SELECT bt.id, bt.post_id, p.tenant_id, bt.locale, bt.body, bt.body_format, bt.updated_at \
         FROM blog_post_translations bt \
         LEFT JOIN blog_posts p ON p.id = bt.post_id \
         WHERE 1 = 1",
    );
    let mut values = Vec::<sea_orm::Value>::new();

    match backend {
        DbBackend::Postgres => {
            if let Some(tenant_id) = tenant_id {
                values.push(tenant_id.into());
                sql.push_str(&format!(" AND p.tenant_id = ${}", values.len()));
            }
            if let Some(cursor) = cursor {
                values.push(cursor.updated_at.clone().into());
                let updated_at_parameter = values.len();
                values.push(cursor.id.into());
                let id_parameter = values.len();
                sql.push_str(&format!(
                    " AND (bt.updated_at > ${updated_at_parameter} OR (bt.updated_at = ${updated_at_parameter} AND bt.id > ${id_parameter}))"
                ));
            }
            values.push((batch_size as i64).into());
            sql.push_str(&format!(
                " ORDER BY bt.updated_at ASC, bt.id ASC LIMIT ${}",
                values.len()
            ));
        }
        DbBackend::Sqlite => {
            if let Some(tenant_id) = tenant_id {
                values.push(tenant_id.into());
                sql.push_str(" AND p.tenant_id = ?");
            }
            if let Some(cursor) = cursor {
                values.push(cursor.updated_at.clone().into());
                values.push(cursor.updated_at.into());
                values.push(cursor.id.into());
                sql.push_str(" AND (bt.updated_at > ? OR (bt.updated_at = ? AND bt.id > ?))");
            }
            values.push((batch_size as i64).into());
            sql.push_str(" ORDER BY bt.updated_at ASC, bt.id ASC LIMIT ?");
        }
        other => bail!("unsupported database backend for Blog backfill: {other:?}"),
    }

    let query = Statement::from_sql_and_values(backend, sql, values);
    let rows = db.query_all(query).await?;
    rows.into_iter()
        .map(|row| {
            Ok(LegacyRow {
                id: row.try_get("", "id")?,
                post_id: row.try_get("", "post_id")?,
                tenant_id: row.try_get("", "tenant_id")?,
                locale: row.try_get("", "locale")?,
                body: row.try_get("", "body")?,
                body_format: row.try_get("", "body_format")?,
                updated_at: row.try_get("", "updated_at")?,
            })
        })
        .collect()
}

async fn optimistic_update(
    txn: &DatabaseTransaction,
    row: &LegacyRow,
    converted_body: &str,
) -> Result<bool> {
    let backend = txn.get_database_backend();
    let (sql, values) = match backend {
        DbBackend::Postgres => (
            "UPDATE blog_post_translations \
             SET body = $1, body_format = $2 \
             WHERE id = $3 AND body = $4 AND body_format = $5 AND updated_at = $6",
            vec![
                converted_body.to_string().into(),
                TARGET_FORMAT.to_string().into(),
                row.id.into(),
                row.body.clone().into(),
                row.body_format.clone().into(),
                row.updated_at.clone().into(),
            ],
        ),
        DbBackend::Sqlite => (
            "UPDATE blog_post_translations \
             SET body = ?, body_format = ? \
             WHERE id = ? AND body = ? AND body_format = ? AND updated_at = ?",
            vec![
                converted_body.to_string().into(),
                TARGET_FORMAT.to_string().into(),
                row.id.into(),
                row.body.clone().into(),
                row.body_format.clone().into(),
                row.updated_at.clone().into(),
            ],
        ),
        other => bail!("unsupported database backend for Blog backfill: {other:?}"),
    };

    let result = txn
        .execute(Statement::from_sql_and_values(backend, sql, values))
        .await?;
    Ok(result.rows_affected() == 1)
}

fn convert_row(row: &LegacyRow, allow_markdown_plain_text: bool) -> Result<Conversion> {
    if row.tenant_id.is_none() {
        bail!(
            "Blog translation {} references missing post {}; repair the owner relation before backfill",
            row.id,
            row.post_id
        );
    }
    let source_format = row.body_format.trim().to_ascii_lowercase();
    let (document, kind) = match source_format.as_str() {
        TARGET_FORMAT => (
            parse_root_document(&row.body).context("invalid canonical richtext root")?,
            ConversionKind::NormalizeRichtext,
        ),
        "rt_json_v1" | "rt_json" => (
            parse_legacy_envelope(&row.body, &row.locale)?,
            ConversionKind::LegacyEnvelope,
        ),
        "markdown" => {
            if !allow_markdown_plain_text {
                bail!(
                    "Markdown conversion is intentionally opt-in; inspect the row and rerun with --allow-markdown-plain-text to preserve it as literal paragraph text"
                );
            }
            (
                article_document_from_plain_text(&row.body),
                ConversionKind::MarkdownPlainText,
            )
        }
        other => bail!(
            "unsupported legacy Blog body_format '{other}'; convert this row manually before the cutover"
        ),
    };

    let normalized = normalize_article(document)
        .map_err(|error| anyhow::anyhow!("Article profile rejected the document: {error}"))?;
    let body = canonical_article_body(&normalized)
        .map_err(|error| anyhow::anyhow!("failed to serialize canonical article: {error}"))?;
    let kind = if source_format == TARGET_FORMAT && body == row.body {
        ConversionKind::AlreadyCanonical
    } else {
        kind
    };

    Ok(Conversion { body, kind })
}

fn parse_root_document(body: &str) -> Result<RichTextDocument> {
    serde_json::from_str(body).context("body is not a RichTextDocument JSON root")
}

fn parse_legacy_envelope(body: &str, row_locale: &str) -> Result<RichTextDocument> {
    let payload: JsonValue = serde_json::from_str(body).context("legacy body is not valid JSON")?;

    if let Some(version) = payload.get("version").and_then(JsonValue::as_str) {
        if version != "rt_json_v1" {
            bail!("unsupported legacy richtext envelope version '{version}'");
        }
    }
    if let Some(locale) = payload.get("locale").and_then(JsonValue::as_str) {
        if locale != row_locale {
            bail!(
                "legacy envelope locale '{locale}' does not match translation locale '{row_locale}'"
            );
        }
    }

    let document = payload.get("doc").cloned().unwrap_or(payload);
    serde_json::from_value(document).context("legacy envelope doc is not a RichTextDocument")
}

fn parse_args<I>(args: I) -> Result<Cli>
where
    I: IntoIterator<Item = String>,
{
    let mut tenant_id = None;
    let mut batch_size = DEFAULT_BATCH_SIZE;
    let mut apply = false;
    let mut dry_run = false;
    let mut allow_markdown_plain_text = false;
    let mut report = None;
    let mut help = false;

    for arg in args {
        match arg.as_str() {
            "--apply" => apply = true,
            "--dry-run" => dry_run = true,
            "--allow-markdown-plain-text" => allow_markdown_plain_text = true,
            "--help" | "-h" => help = true,
            _ => {
                if let Some(value) = arg.strip_prefix("--tenant-id=") {
                    tenant_id = Some(Uuid::parse_str(value).context("invalid --tenant-id")?);
                } else if let Some(value) = arg.strip_prefix("--batch-size=") {
                    batch_size = value.parse().context("invalid --batch-size")?;
                } else if let Some(value) = arg.strip_prefix("--report=") {
                    if value.trim().is_empty() {
                        bail!("--report path cannot be empty");
                    }
                    report = Some(PathBuf::from(value));
                } else {
                    bail!("unknown argument: {arg}");
                }
            }
        }
    }

    if apply && dry_run {
        bail!("--apply and --dry-run are mutually exclusive");
    }
    if !(1..=MAX_BATCH_SIZE).contains(&batch_size) {
        bail!("--batch-size must be between 1 and {MAX_BATCH_SIZE}");
    }

    Ok(Cli {
        tenant_id,
        batch_size,
        apply,
        allow_markdown_plain_text,
        report,
        help,
    })
}

fn print_metrics(stage: &str, metrics: &Metrics) {
    println!(
        "{stage}: scanned={} already_canonical={} planned_updates={} applied={} invalid={}",
        metrics.scanned,
        metrics.already_canonical,
        metrics.planned_updates,
        metrics.applied,
        metrics.invalid
    );
}

fn print_usage() {
    println!(
        "Blog article richtext offline backfill\n\
         \n\
         Default mode is dry-run and never writes rows.\n\
         \n\
         DATABASE_URL=postgresql://... cargo run -p rustok-blog --bin blog_article_richtext_backfill -- \\\n           --tenant-id=<uuid> --report=artifacts/blog-richtext-preflight.ndjson\n\
         \n\
         DATABASE_URL=postgresql://... cargo run -p rustok-blog --bin blog_article_richtext_backfill -- \\\n           --tenant-id=<uuid> --apply --allow-markdown-plain-text\n\
         \n\
         Flags:\n\
           --apply                         write only after a full successful preflight\n\
           --dry-run                       explicit alias for the default mode\n\
           --allow-markdown-plain-text     preserve Markdown rows as literal paragraph text\n\
           --tenant-id=<uuid>              restrict the scan to one tenant\n\
           --batch-size=<1..10000>         default: 500\n\
           --report=<path>                 write NDJSON preflight records without content bodies\n"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(format: &str, body: &str, locale: &str) -> LegacyRow {
        LegacyRow {
            id: Uuid::new_v4(),
            post_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            locale: locale.to_string(),
            body: body.to_string(),
            body_format: format.to_string(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn markdown_conversion_requires_explicit_lossy_opt_in() {
        let source = row("markdown", "First line\nsecond line\n\nNext", "en");
        assert!(convert_row(&source, false).is_err());

        let converted = convert_row(&source, true).expect("opted-in conversion");
        let document: RichTextDocument = serde_json::from_str(&converted.body).expect("document");
        assert_eq!(document.content.len(), 2);
        assert_eq!(
            document.content[0].content[0].text.as_deref(),
            Some("First line second line")
        );
    }

    #[test]
    fn legacy_envelope_extracts_the_root_document() {
        let source = row(
            "rt_json_v1",
            &serde_json::json!({
                "version": "rt_json_v1",
                "locale": "ru",
                "doc": {
                    "type": "doc",
                    "content": [{
                        "type": "paragraph",
                        "content": [{"type": "text", "text": "Привет"}]
                    }]
                }
            })
            .to_string(),
            "ru",
        );

        let converted = convert_row(&source, false).expect("legacy envelope");
        let document: RichTextDocument = serde_json::from_str(&converted.body).expect("document");
        assert_eq!(document.kind, "doc");
        assert!(!converted.body.contains("rt_json_v1"));
    }

    #[test]
    fn legacy_envelope_locale_mismatch_fails_closed() {
        let source = row(
            "rt_json_v1",
            &serde_json::json!({
                "version": "rt_json_v1",
                "locale": "en",
                "doc": {"type": "doc", "content": []}
            })
            .to_string(),
            "ru",
        );
        assert!(convert_row(&source, false).is_err());
    }

    #[test]
    fn unknown_format_fails_closed() {
        let source = row("grapesjs", "{}", "en");
        assert!(convert_row(&source, true).is_err());
    }

    #[test]
    fn cli_is_dry_run_by_default() {
        let cli = parse_args(Vec::<String>::new()).expect("cli");
        assert!(!cli.apply);
        assert_eq!(cli.batch_size, DEFAULT_BATCH_SIZE);
    }
}
