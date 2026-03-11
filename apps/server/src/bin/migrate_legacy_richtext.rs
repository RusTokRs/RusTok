use std::{env, fs, path::PathBuf, str::FromStr, time::Duration};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rustok_content::entities::{body, node};
use rustok_core::{validate_and_sanitize_rt_json, RtJsonValidationConfig};
use sea_orm::{
    sea_query::Expr, ColumnTrait, Condition, Database, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const TARGET_KINDS: &[&str] = &["post", "comment", "forum_topic", "forum_reply"];

#[derive(Debug, Default)]
struct Metrics {
    processed: u64,
    succeeded: u64,
    failed: u64,
    skipped: u64,
}

#[derive(Debug, Clone)]
struct Cli {
    tenant_id: Uuid,
    batch_size: u64,
    max_retries: u32,
    retry_delay_ms: u64,
    dry_run: bool,
    checkpoint_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Checkpoint {
    last_updated_at: DateTime<Utc>,
    last_body_id: Uuid,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = parse_args()?;
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let db = Database::connect(&database_url).await?;

    let mut checkpoint = load_checkpoint(&cli.checkpoint_file)?;
    let mut metrics = Metrics::default();

    loop {
        let rows = fetch_batch(&db, cli.tenant_id, checkpoint.as_ref(), cli.batch_size).await?;
        if rows.is_empty() {
            break;
        }

        for (body_row, _node_row) in rows {
            metrics.processed += 1;
            let current_checkpoint = Checkpoint {
                last_updated_at: body_row.updated_at.into(),
                last_body_id: body_row.id,
            };

            match migrate_one(&db, &cli, &body_row).await {
                Ok(MigrationOutcome::Succeeded) => metrics.succeeded += 1,
                Ok(MigrationOutcome::Skipped) => metrics.skipped += 1,
                Err(err) => {
                    metrics.failed += 1;
                    eprintln!(
                        "[failed] body_id={} locale={} err={:#}",
                        body_row.id, body_row.locale, err
                    );
                }
            }

            checkpoint = Some(current_checkpoint.clone());
            persist_checkpoint(&cli.checkpoint_file, &current_checkpoint)?;
        }
    }

    println!(
        "done tenant={} dry_run={} processed={} succeeded={} failed={} skipped={}",
        cli.tenant_id,
        cli.dry_run,
        metrics.processed,
        metrics.succeeded,
        metrics.failed,
        metrics.skipped
    );

    if metrics.failed > 0 {
        anyhow::bail!("migration finished with {} failed records", metrics.failed);
    }

    Ok(())
}

#[derive(Debug)]
enum MigrationOutcome {
    Succeeded,
    Skipped,
}

async fn migrate_one(
    db: &sea_orm::DatabaseConnection,
    cli: &Cli,
    row: &body::Model,
) -> Result<MigrationOutcome> {
    if row.format == "rt_json_v1" {
        return Ok(MigrationOutcome::Skipped);
    }

    let markdown = match &row.body {
        Some(value) if !value.trim().is_empty() => value,
        _ => return Ok(MigrationOutcome::Skipped),
    };

    let payload = markdown_to_rt_json(markdown, &row.locale);
    let validation =
        validate_and_sanitize_rt_json(&payload, &RtJsonValidationConfig::for_locale(&row.locale))
            .map_err(|e| anyhow::anyhow!("rt_json validation failed: {e}"))?;
    let sanitized = serde_json::to_string(&validation.sanitized)?;

    if cli.dry_run {
        return Ok(MigrationOutcome::Succeeded);
    }

    for _attempt in 0..=cli.max_retries {
        let update = body::Entity::update_many()
            .col_expr(body::Column::Body, Expr::value(Some(sanitized.clone())))
            .col_expr(body::Column::Format, Expr::value("rt_json_v1"))
            .col_expr(body::Column::UpdatedAt, Expr::value(Utc::now()))
            .filter(body::Column::Id.eq(row.id))
            .filter(body::Column::Format.eq("markdown"))
            .filter(body::Column::UpdatedAt.eq(row.updated_at))
            .exec(db)
            .await?;

        if update.rows_affected == 1 {
            return Ok(MigrationOutcome::Succeeded);
        }

        if let Some(current) = body::Entity::find_by_id(row.id).one(db).await? {
            if current.format == "rt_json_v1" {
                return Ok(MigrationOutcome::Skipped);
            }
        }

        tokio::time::sleep(Duration::from_millis(cli.retry_delay_ms)).await;
    }

    anyhow::bail!("exhausted retries for body_id={}", row.id)
}

async fn fetch_batch(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    checkpoint: Option<&Checkpoint>,
    batch_size: u64,
) -> Result<Vec<(body::Model, Option<node::Model>)>> {
    let mut query = body::Entity::find()
        .find_also_related(node::Entity)
        .filter(body::Column::Format.eq("markdown"))
        .filter(node::Column::TenantId.eq(tenant_id))
        .filter(node::Column::Kind.is_in(TARGET_KINDS.iter().copied()))
        .order_by_asc(body::Column::UpdatedAt)
        .order_by_asc(body::Column::Id)
        .limit(batch_size);

    if let Some(cp) = checkpoint {
        let cursor = Condition::any()
            .add(body::Column::UpdatedAt.gt(cp.last_updated_at))
            .add(
                Condition::all()
                    .add(body::Column::UpdatedAt.eq(cp.last_updated_at))
                    .add(body::Column::Id.gt(cp.last_body_id)),
            );
        query = query.filter(cursor);
    }

    let rows = query.all(db).await?;
    Ok(rows)
}

fn markdown_to_rt_json(markdown: &str, locale: &str) -> Value {
    let mut content = Vec::new();

    for paragraph in markdown.split("\n\n") {
        let trimmed = paragraph.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lines: Vec<&str> = trimmed.lines().collect();
        let mut paragraph_nodes = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            let line = line.trim_end();
            if !line.is_empty() {
                paragraph_nodes.push(json!({"type": "text", "text": line}));
            }
            if idx + 1 < lines.len() {
                paragraph_nodes.push(json!({"type": "hardBreak"}));
            }
        }

        if paragraph_nodes.is_empty() {
            continue;
        }

        content.push(json!({"type": "paragraph", "content": paragraph_nodes}));
    }

    if content.is_empty() {
        content.push(json!({"type": "paragraph", "content": []}));
    }

    json!({
        "version": "rt_json_v1",
        "locale": locale,
        "doc": {
            "type": "doc",
            "content": content
        }
    })
}

fn parse_args() -> Result<Cli> {
    let mut tenant_id = None;
    let mut dry_run = false;
    let mut batch_size = 100u64;
    let mut max_retries = 3u32;
    let mut retry_delay_ms = 200u64;
    let mut checkpoint_file = PathBuf::from("scripts/checkpoints/legacy_richtext.json");

    let args: Vec<String> = env::args().collect();
    for arg in args.iter().skip(1) {
        if arg == "--dry-run" {
            dry_run = true;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--tenant-id=") {
            tenant_id = Some(Uuid::from_str(value).context("invalid --tenant-id")?);
            continue;
        }
        if let Some(value) = arg.strip_prefix("--batch-size=") {
            batch_size = value.parse().context("invalid --batch-size")?;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--max-retries=") {
            max_retries = value.parse().context("invalid --max-retries")?;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--retry-delay-ms=") {
            retry_delay_ms = value.parse().context("invalid --retry-delay-ms")?;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--checkpoint-file=") {
            checkpoint_file = PathBuf::from(value);
            continue;
        }

        anyhow::bail!("unknown argument: {arg}");
    }

    let tenant_id = tenant_id.context("--tenant-id=<uuid> is required")?;

    Ok(Cli {
        tenant_id,
        batch_size,
        max_retries,
        retry_delay_ms,
        dry_run,
        checkpoint_file,
    })
}

fn load_checkpoint(path: &PathBuf) -> Result<Option<Checkpoint>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read checkpoint file {}", path.display()))?;
    let checkpoint: Checkpoint = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse checkpoint file {}", path.display()))?;
    Ok(Some(checkpoint))
}

fn persist_checkpoint(path: &PathBuf, checkpoint: &Checkpoint) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create checkpoint dir {}", parent.display()))?;
    }

    let payload = serde_json::to_string_pretty(checkpoint)?;
    fs::write(path, payload)
        .with_context(|| format!("failed to write checkpoint {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_conversion_has_rt_json_v1_contract() {
        let converted = markdown_to_rt_json("line 1\nline2\n\nnext", "ru");
        assert_eq!(converted["version"], "rt_json_v1");
        assert_eq!(converted["locale"], "ru");
        assert!(converted["doc"]["content"].as_array().unwrap().len() >= 2);
    }
}
