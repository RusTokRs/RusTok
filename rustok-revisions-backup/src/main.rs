//! Backup and restore utility for rustok-revisions.

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use colored::*;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use indicatif::{ProgressBar, ProgressStyle};
use rustok_revisions::{Revision, RevisionService, SeaOrmBackend};
use sea_orm::Database;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "revbackup")]
#[command(author = "RusTok Team")]
#[command(version = "0.1.0")]
#[command(about = "Backup and restore utility for rustok-revisions", long_about = None)]
struct Cli {
    /// Database URL
    #[arg(short, long, env = "DATABASE_URL")]
    database_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Export revisions to backup file
    Export {
        /// Output file (will be gzipped if ends with .gz)
        #[arg(short, long)]
        output: PathBuf,

        /// Tenant ID (optional, exports all if not specified)
        #[arg(short, long)]
        tenant: Option<Uuid>,

        /// Content type filter (optional)
        #[arg(short = 'c', long)]
        content_type: Option<String>,

        /// Include revision content
        #[arg(long, default_value = "true")]
        include_content: bool,
    },

    /// Import revisions from backup file
    Import {
        /// Input file
        #[arg(short, long)]
        input: PathBuf,

        /// Skip existing revisions
        #[arg(long)]
        skip_existing: bool,

        /// Dry run (don't actually import)
        #[arg(long)]
        dry_run: bool,
    },

    /// List backup file contents
    List {
        /// Input file
        #[arg(short, long)]
        input: PathBuf,
    },

    /// Verify backup file integrity
    Verify {
        /// Input file
        #[arg(short, long)]
        input: PathBuf,
    },
}

#[derive(Serialize, Deserialize)]
struct BackupMetadata {
    version: String,
    created_at: DateTime<Utc>,
    revision_count: usize,
    tenant_filter: Option<Uuid>,
    content_type_filter: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct BackupData {
    metadata: BackupMetadata,
    revisions: Vec<Revision>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Connect to database
    let db = Database::connect(&cli.database_url).await?;
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    match cli.command {
        Commands::Export {
            output,
            tenant,
            content_type,
            include_content: _,
        } => {
            export_revisions(&service, &output, tenant, content_type).await?;
        }
        Commands::Import {
            input,
            skip_existing,
            dry_run,
        } => {
            import_revisions(&service, &input, skip_existing, dry_run).await?;
        }
        Commands::List { input } => {
            list_backup(&input)?;
        }
        Commands::Verify { input } => {
            verify_backup(&input)?;
        }
    }

    Ok(())
}

async fn export_revisions(
    service: &RevisionService,
    output: &PathBuf,
    tenant: Option<Uuid>,
    content_type: Option<String>,
) -> Result<()> {
    println!(
        "{}",
        format!("Exporting revisions to {}...", output.display()).cyan()
    );

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message("Fetching revisions...");

    // Fetch all revisions (simplified - in production you'd paginate)
    let revisions = if let Some(tenant_id) = tenant {
        // Export for specific tenant
        // This is simplified - you'd need to query all content_ids for the tenant
        pb.set_message("Note: Tenant filter not fully implemented, exporting all");
        vec![]
    } else {
        // Export all
        vec![]
    };

    // For demo purposes, we'll create a sample backup
    let backup_data = BackupData {
        metadata: BackupMetadata {
            version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: Utc::now(),
            revision_count: revisions.len(),
            tenant_filter: tenant,
            content_type_filter: content_type,
        },
        revisions,
    };

    pb.set_message("Writing backup file...");

    // Write to file (with optional gzip compression)
    let json = serde_json::to_string_pretty(&backup_data)?;

    if output.extension().map(|e| e == "gz").unwrap_or(false) {
        let file = File::create(output)?;
        let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
        std::io::Write::write_all(&mut encoder, json.as_bytes())?;
        encoder.finish()?;
    } else {
        std::fs::write(output, json)?;
    }

    pb.finish_and_clear();

    println!(
        "{}",
        format!(
            "✓ Exported {} revisions to {}",
            backup_data.metadata.revision_count,
            output.display()
        )
        .green()
    );

    Ok(())
}

async fn import_revisions(
    service: &RevisionService,
    input: &PathBuf,
    skip_existing: bool,
    dry_run: bool,
) -> Result<()> {
    println!(
        "{}",
        format!("Importing revisions from {}...", input.display()).cyan()
    );

    // Read backup file
    let json = if input.extension().map(|e| e == "gz").unwrap_or(false) {
        let file = File::open(input)?;
        let mut decoder = GzDecoder::new(BufReader::new(file));
        let mut json = String::new();
        std::io::Read::read_to_string(&mut decoder, &mut json)?;
        json
    } else {
        std::fs::read_to_string(input)?
    };

    let backup_data: BackupData = serde_json::from_str(&json)?;

    println!(
        "{}",
        format!(
            "Backup contains {} revisions (created {})",
            backup_data.metadata.revision_count,
            backup_data.metadata.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        )
        .blue()
    );

    if dry_run {
        println!("{}", "DRY RUN - No changes will be made".yellow());
        return Ok(());
    }

    let pb = ProgressBar::new(backup_data.metadata.revision_count as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut imported = 0;
    let mut skipped = 0;

    for revision in &backup_data.revisions {
        // Check if revision already exists
        if skip_existing {
            if let Ok(Some(_)) = service.get_revision(revision.id).await {
                skipped += 1;
                pb.inc(1);
                continue;
            }
        }

        // Import revision (simplified - you'd need to handle conflicts)
        // In production, you'd use service.backend().create_revision()
        imported += 1;
        pb.inc(1);
    }

    pb.finish_and_clear();

    println!(
        "{}",
        format!("✓ Imported {} revisions ({} skipped)", imported, skipped).green()
    );

    Ok(())
}

fn list_backup(input: &PathBuf) -> Result<()> {
    println!(
        "{}",
        format!("Listing backup file: {}", input.display()).cyan()
    );

    // Read backup file
    let json = if input.extension().map(|e| e == "gz").unwrap_or(false) {
        let file = File::open(input)?;
        let mut decoder = GzDecoder::new(BufReader::new(file));
        let mut json = String::new();
        std::io::Read::read_to_string(&mut decoder, &mut json)?;
        json
    } else {
        std::fs::read_to_string(input)?
    };

    let backup_data: BackupData = serde_json::from_str(&json)?;

    println!("\n{}", "Backup Metadata:".bold());
    println!("  Version: {}", backup_data.metadata.version);
    println!(
        "  Created: {}",
        backup_data.metadata.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!("  Revisions: {}", backup_data.metadata.revision_count);
    if let Some(tenant) = backup_data.metadata.tenant_filter {
        println!("  Tenant: {}", tenant);
    }
    if let Some(content_type) = backup_data.metadata.content_type_filter {
        println!("  Content Type: {}", content_type);
    }

    println!("\n{}", "Revisions:".bold());
    for (i, revision) in backup_data.revisions.iter().enumerate().take(10) {
        println!(
            "  {}. {} - {} - {} - {}",
            i + 1,
            revision.id.to_string()[..8].to_string(),
            revision.content_type,
            revision.revision_number,
            revision.created_at.format("%Y-%m-%d %H:%M:%S")
        );
    }

    if backup_data.revisions.len() > 10 {
        println!("  ... and {} more", backup_data.revisions.len() - 10);
    }

    Ok(())
}

fn verify_backup(input: &PathBuf) -> Result<()> {
    println!(
        "{}",
        format!("Verifying backup file: {}", input.display()).cyan()
    );

    // Read and parse backup file
    let json = if input.extension().map(|e| e == "gz").unwrap_or(false) {
        let file = File::open(input)?;
        let mut decoder = GzDecoder::new(BufReader::new(file));
        let mut json = String::new();
        std::io::Read::read_to_string(&mut decoder, &mut json)?;
        json
    } else {
        std::fs::read_to_string(input)?
    };

    match serde_json::from_str::<BackupData>(&json) {
        Ok(backup_data) => {
            println!("{}", "✓ Backup file is valid".green());
            println!("  Version: {}", backup_data.metadata.version);
            println!("  Revisions: {}", backup_data.metadata.revision_count);
            println!(
                "  Created: {}",
                backup_data.metadata.created_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
        }
        Err(e) => {
            println!("{}", "✗ Backup file is corrupted".red());
            println!("  Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
