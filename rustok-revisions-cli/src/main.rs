//! CLI tool for managing content revisions.

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use rustok_revisions::{RevisionService, SeaOrmBackend};
use sea_orm::Database;
use uuid::Uuid;

mod commands;

#[derive(Parser)]
#[command(name = "revctl")]
#[command(author = "RusTok Team")]
#[command(version = "0.1.0")]
#[command(about = "CLI tool for managing content revisions", long_about = None)]
struct Cli {
    /// Database URL
    #[arg(short, long, env = "DATABASE_URL")]
    database_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List revisions for content
    List {
        /// Tenant ID
        #[arg(short, long)]
        tenant: Uuid,

        /// Content ID
        #[arg(short, long)]
        content: Uuid,

        /// Locale
        #[arg(short, long, default_value = "en")]
        locale: String,

        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Show revision details
    Show {
        /// Revision ID
        revision_id: Uuid,
    },

    /// Compare two revisions
    Diff {
        /// First revision ID
        from: Uuid,

        /// Second revision ID
        to: Uuid,
    },

    /// Restore content to a revision
    Restore {
        /// Tenant ID
        #[arg(short, long)]
        tenant: Uuid,

        /// Content ID
        #[arg(short, long)]
        content: Uuid,

        /// Locale
        #[arg(short, long, default_value = "en")]
        locale: String,

        /// Revision ID to restore to
        revision_id: Uuid,

        /// User ID performing the restore
        #[arg(short, long)]
        user: Uuid,

        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Create named version
    Tag {
        /// Tenant ID
        #[arg(short, long)]
        tenant: Uuid,

        /// Content ID
        #[arg(short, long)]
        content: Uuid,

        /// Locale
        #[arg(short, long, default_value = "en")]
        locale: String,

        /// Revision ID to tag
        revision_id: Uuid,

        /// Version name
        #[arg(short = 'n', long)]
        name: String,
    },

    /// List named versions
    Tags {
        /// Tenant ID
        #[arg(short, long)]
        tenant: Uuid,

        /// Content ID
        #[arg(short, long)]
        content: Uuid,

        /// Locale
        #[arg(short, long, default_value = "en")]
        locale: String,
    },

    /// Count revisions
    Count {
        /// Tenant ID
        #[arg(short, long)]
        tenant: Uuid,

        /// Content ID
        #[arg(short, long)]
        content: Uuid,

        /// Locale
        #[arg(short, long, default_value = "en")]
        locale: String,
    },

    /// Cleanup old revisions
    Cleanup {
        /// Tenant ID
        #[arg(short, long)]
        tenant: Uuid,

        /// Content ID
        #[arg(short, long)]
        content: Uuid,

        /// Locale
        #[arg(short, long, default_value = "en")]
        locale: String,

        /// Keep last N revisions
        #[arg(short, long)]
        keep: usize,

        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Export revisions to JSON
    Export {
        /// Tenant ID
        #[arg(short, long)]
        tenant: Uuid,

        /// Content ID
        #[arg(short, long)]
        content: Uuid,

        /// Locale
        #[arg(short, long, default_value = "en")]
        locale: String,

        /// Output file
        #[arg(short, long)]
        output: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Connect to database
    let db = Database::connect(&cli.database_url).await?;
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));

    match cli.command {
        Commands::List {
            tenant,
            content,
            locale,
            limit,
        } => {
            commands::list_revisions(&service, tenant, content, &locale, limit).await?;
        }
        Commands::Show { revision_id } => {
            commands::show_revision(&service, revision_id).await?;
        }
        Commands::Diff { from, to } => {
            commands::diff_revisions(&service, from, to).await?;
        }
        Commands::Restore {
            tenant,
            content,
            locale,
            revision_id,
            user,
            yes,
        } => {
            commands::restore_revision(&service, tenant, content, &locale, revision_id, user, yes)
                .await?;
        }
        Commands::Tag {
            tenant,
            content,
            locale,
            revision_id,
            name,
        } => {
            commands::create_tag(&service, tenant, content, &locale, revision_id, &name).await?;
        }
        Commands::Tags {
            tenant,
            content,
            locale,
        } => {
            commands::list_tags(&service, tenant, content, &locale).await?;
        }
        Commands::Count {
            tenant,
            content,
            locale,
        } => {
            commands::count_revisions(&service, tenant, content, &locale).await?;
        }
        Commands::Cleanup {
            tenant,
            content,
            locale,
            keep,
            yes,
        } => {
            commands::cleanup_revisions(&service, tenant, content, &locale, keep, yes).await?;
        }
        Commands::Export {
            tenant,
            content,
            locale,
            output,
        } => {
            commands::export_revisions(&service, tenant, content, &locale, &output).await?;
        }
    }

    Ok(())
}
