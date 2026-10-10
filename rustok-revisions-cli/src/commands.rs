//! CLI command implementations.

use anyhow::Result;
use colored::*;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use rustok_revisions::{ChangeSource, RetentionPolicy, RevisionService};
use serde_json::json;
use std::fs::File;
use std::io::Write;
use tabled::{Table, Tabled};
use uuid::Uuid;

#[derive(Tabled)]
struct RevisionRow {
    #[tabled(rename = "#")]
    number: i64,
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Event")]
    event: String,
    #[tabled(rename = "Created")]
    created: String,
    #[tabled(rename = "User")]
    user: String,
    #[tabled(rename = "Tag")]
    tag: String,
}

pub async fn list_revisions(
    service: &RevisionService,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    limit: Option<usize>,
) -> Result<()> {
    println!(
        "{}",
        format!(
            "Listing revisions for content {} (locale: {})",
            content_id, locale
        )
        .bold()
    );

    let revisions = service
        .list_revisions(tenant_id, content_id, locale, limit, None)
        .await?;

    if revisions.is_empty() {
        println!("{}", "No revisions found".yellow());
        return Ok(());
    }

    let rows: Vec<RevisionRow> = revisions
        .iter()
        .map(|r| RevisionRow {
            number: r.revision_number,
            id: r.id.to_string()[..8].to_string(),
            event: format!("{:?}", r.event),
            created: r.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            user: r.user_id.to_string()[..8].to_string(),
            tag: r.version_name.clone().unwrap_or_else(|| "-".to_string()),
        })
        .collect();

    let table = Table::new(rows);
    println!("{}", table);
    println!("\nTotal: {} revisions", revisions.len());

    Ok(())
}

pub async fn show_revision(service: &RevisionService, revision_id: Uuid) -> Result<()> {
    let revision = service
        .get_revision(revision_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Revision not found"))?;

    println!("{}", "Revision Details".bold().underline());
    println!("{}: {}", "ID".cyan(), revision.id);
    println!("{}: {}", "Revision #".cyan(), revision.revision_number);
    println!("{}: {}", "Tenant ID".cyan(), revision.tenant_id);
    println!("{}: {}", "Content ID".cyan(), revision.content_id);
    println!("{}: {}", "Content Type".cyan(), revision.content_type);
    println!("{}: {}", "Locale".cyan(), revision.locale);
    println!("{}: {:?}", "Event".cyan(), revision.event);
    println!(
        "{}: {}",
        "Created At".cyan(),
        revision.created_at.format("%Y-%m-%d %H:%M:%S %Z")
    );
    println!("{}: {}", "User ID".cyan(), revision.user_id);
    println!("{}: {:?}", "Source".cyan(), revision.source);
    println!(
        "{}: {}",
        "Summary".cyan(),
        revision.summary.as_deref().unwrap_or("-")
    );
    println!(
        "{}: {}",
        "IP Address".cyan(),
        revision.ip_address.as_deref().unwrap_or("-")
    );
    println!(
        "{}: {}",
        "User Agent".cyan(),
        revision.user_agent.as_deref().unwrap_or("-")
    );
    println!(
        "{}: {}",
        "Version Name".cyan(),
        revision.version_name.as_deref().unwrap_or("-")
    );

    if let Some(parent_id) = revision.parent_revision_id {
        println!("{}: {}", "Parent Revision".cyan(), parent_id);
    }

    println!("\n{}", "Content".bold().underline());
    println!(
        "{}",
        serde_json::to_string_pretty(&revision.content)?
    );

    if !revision.custom_metadata.is_null() {
        println!("\n{}", "Custom Metadata".bold().underline());
        println!(
            "{}",
            serde_json::to_string_pretty(&revision.custom_metadata)?
        );
    }

    Ok(())
}

pub async fn diff_revisions(
    service: &RevisionService,
    from_id: Uuid,
    to_id: Uuid,
) -> Result<()> {
    println!(
        "{}",
        format!("Comparing revisions {} → {}", from_id, to_id).bold()
    );

    let diff = service.compare_revisions(from_id, to_id).await?;

    println!("\n{}", "Added Fields".green().bold());
    if diff.added.is_object() && diff.added.as_object().unwrap().is_empty() {
        println!("{}", "  (none)".dimmed());
    } else {
        for (key, value) in diff.added.as_object().unwrap() {
            println!("  {} {}: {}", "+".green(), key.green(), value);
        }
    }

    println!("\n{}", "Removed Fields".red().bold());
    if diff.removed.is_object() && diff.removed.as_object().unwrap().is_empty() {
        println!("{}", "  (none)".dimmed());
    } else {
        for (key, value) in diff.removed.as_object().unwrap() {
            println!("  {} {}: {}", "-".red(), key.red(), value);
        }
    }

    println!("\n{}", "Changed Fields".yellow().bold());
    if diff.changed.is_object() && diff.changed.as_object().unwrap().is_empty() {
        println!("{}", "  (none)".dimmed());
    } else {
        for (key, change) in diff.changed.as_object().unwrap() {
            println!("  {} {}:", "~".yellow(), key.yellow());
            if let Some(from) = change.get("from") {
                println!("    {} {}", "from:".red(), from);
            }
            if let Some(to) = change.get("to") {
                println!("    {} {}", "to:".green(), to);
            }
        }
    }

    Ok(())
}

pub async fn restore_revision(
    service: &RevisionService,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    revision_id: Uuid,
    user_id: Uuid,
    yes: bool,
) -> Result<()> {
    if !yes {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to restore content {} to revision {}?",
                content_id, revision_id
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{}", "Restore cancelled".yellow());
            return Ok(());
        }
    }

    println!("{}", "Restoring revision...".cyan());

    let (restored_content, new_revision) = service
        .restore_revision::<serde_json::Value>(
            tenant_id,
            content_id,
            locale,
            revision_id,
            user_id,
            ChangeSource::Custom("cli".to_string()),
        )
        .await?;

    println!(
        "{}",
        format!(
            "✓ Restored to revision {} (new revision #{})",
            revision_id, new_revision.revision_number
        )
        .green()
    );

    println!("\n{}", "Restored Content".bold().underline());
    println!("{}", serde_json::to_string_pretty(&restored_content)?);

    Ok(())
}

pub async fn create_tag(
    service: &RevisionService,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    revision_id: Uuid,
    name: &str,
) -> Result<()> {
    service
        .create_named_version(tenant_id, content_id, locale, revision_id, name)
        .await?;

    println!(
        "{}",
        format!("✓ Created tag '{}' for revision {}", name, revision_id).green()
    );

    Ok(())
}

pub async fn list_tags(
    service: &RevisionService,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
) -> Result<()> {
    println!(
        "{}",
        format!("Named versions for content {}", content_id).bold()
    );

    let tags = service
        .get_named_versions(tenant_id, content_id, locale)
        .await?;

    if tags.is_empty() {
        println!("{}", "No named versions found".yellow());
        return Ok(());
    }

    for tag in tags {
        println!(
            "  {} {} (revision #{}) - {}",
            "•".cyan(),
            tag.version_name.as_deref().unwrap_or("unnamed").bold(),
            tag.revision_number,
            tag.created_at.format("%Y-%m-%d %H:%M:%S")
        );
    }

    println!("\nTotal: {} named versions", tags.len());

    Ok(())
}

pub async fn count_revisions(
    service: &RevisionService,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
) -> Result<()> {
    let count = service
        .count_revisions(tenant_id, content_id, locale)
        .await?;

    println!(
        "{}",
        format!("Total revisions: {}", count).bold()
    );

    Ok(())
}

pub async fn cleanup_revisions(
    service: &RevisionService,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    keep: usize,
    yes: bool,
) -> Result<()> {
    if !yes {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to delete old revisions, keeping only the last {}?",
                keep
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{}", "Cleanup cancelled".yellow());
            return Ok(());
        }
    }

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message("Cleaning up old revisions...");

    let deleted = service
        .apply_retention_policy_for_type::<serde_json::Value>(
            tenant_id,
            content_id,
            locale,
            &RetentionPolicy::KeepLast(keep),
        )
        .await?;

    pb.finish_and_clear();

    println!(
        "{}",
        format!("✓ Deleted {} old revisions, kept {}", deleted, keep).green()
    );

    Ok(())
}

pub async fn export_revisions(
    service: &RevisionService,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    output: &str,
) -> Result<()> {
    println!(
        "{}",
        format!("Exporting revisions to {}...", output).cyan()
    );

    let revisions = service
        .list_revisions(tenant_id, content_id, locale, None, None)
        .await?;

    let export_data = json!({
        "tenant_id": tenant_id,
        "content_id": content_id,
        "locale": locale,
        "exported_at": chrono::Utc::now(),
        "revision_count": revisions.len(),
        "revisions": revisions.iter().map(|r| json!({
            "id": r.id,
            "revision_number": r.revision_number,
            "content_type": r.content_type,
            "event": format!("{:?}", r.event),
            "content": r.content,
            "user_id": r.user_id,
            "source": format!("{:?}", r.source),
            "summary": r.summary,
            "created_at": r.created_at,
            "version_name": r.version_name,
        })).collect::<Vec<_>>()
    });

    let mut file = File::create(output)?;
    file.write_all(serde_json::to_string_pretty(&export_data)?.as_bytes())?;

    println!(
        "{}",
        format!("✓ Exported {} revisions to {}", revisions.len(), output).green()
    );

    Ok(())
}
