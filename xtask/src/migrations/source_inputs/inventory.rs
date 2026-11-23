use super::{check_commit, fingerprint, read_declarations, write_report, MigrationInputs};
use crate::migrations::{
    digest, git_snapshot, read_source, validate_owner_report, Plan, SourceSnapshot,
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Copy)]
enum Side {
    Reference,
    Candidate,
}

impl Side {
    fn name(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Candidate => "candidate",
        }
    }

    fn source(self, plan: &Plan) -> (&str, &SourceSnapshot) {
        match self {
            Self::Reference => (&plan.reference_commit, &plan.reference),
            Self::Candidate => (&plan.candidate_head, &plan.candidate),
        }
    }
}

#[derive(Deserialize)]
struct FreshEvidence {
    schema_version: u32,
    scope: String,
    source_plan_sha256: String,
    reference_source_sha256: String,
    candidate_source_sha256: String,
    build_identity: BTreeMap<String, BuildIdentity>,
    database_report_sha256: String,
}

#[derive(Deserialize)]
struct BuildIdentity {
    sha256: String,
    evidence: Value,
}

#[derive(Deserialize)]
struct Export {
    schema_version: u32,
    migrations: Vec<String>,
    entries: Vec<Value>,
}

#[derive(Serialize)]
struct BoundEntry {
    declaration: Value,
    source: Option<MigrationInputs>,
}

#[derive(Serialize)]
struct Coverage {
    inventory_steps: usize,
    declared_source_steps: usize,
    local_link_checked_steps: usize,
    transitive_build_inputs_verified: bool,
}

#[derive(Serialize)]
struct Report<'a> {
    schema_version: u32,
    scope: &'static str,
    side: &'static str,
    commit: &'a str,
    source_snapshot_sha256: &'a str,
    source_plan_sha256: &'a str,
    fresh_report_sha256: String,
    build_identity_sha256: &'a str,
    inventory_sha256: String,
    source_declarations_sha256: String,
    coverage: Coverage,
    entries: Vec<BoundEntry>,
}

pub(in crate::migrations) fn bind_inventory(
    root: &Path,
    plan_path: &Path,
    inputs: &Path,
    side: &str,
) -> Result<()> {
    let side = match side {
        "reference" => Side::Reference,
        "candidate" => Side::Candidate,
        _ => bail!("--side must be reference or candidate"),
    };
    let plan_path = root.join(plan_path).canonicalize()?;
    let fresh = plan_path
        .parent()
        .context("source plan directory missing")?
        .join("fresh");
    let plan_bytes = read_source(&plan_path)?;
    let plan: Plan = serde_json::from_slice(&plan_bytes).context("invalid source plan")?;
    let (commit, source) = side.source(&plan);
    let fresh_bytes = read_source(&fresh.join("report.json"))
        .context("completed verify-fresh report required")?;
    let report: FreshEvidence =
        serde_json::from_slice(&fresh_bytes).context("invalid verify-fresh report")?;
    let inventory_bytes = read_source(&fresh.join(format!("{}-plan.json", side.name())))
        .context("executable migration inventory export unavailable")?;
    let inventory_sha256 = digest(&inventory_bytes);
    let database_bytes = read_source(&fresh.join("database-report.json"))
        .context("completed database report required")?;
    let build = validate_build(
        &plan,
        &plan_bytes,
        &report,
        side,
        &inventory_sha256,
        &database_bytes,
    )?;
    let inventory = read_inventory(&inventory_bytes)?;
    let (declarations, declaration_bytes) = read_declarations(&root.join(inputs))?;
    let inventory_names: BTreeSet<_> = inventory.migrations.iter().map(String::as_str).collect();
    for migration in declarations.keys() {
        ensure!(
            inventory_names.contains(migration.as_str()),
            "source declaration for {migration} is absent from the executable inventory"
        );
    }
    check_commit(root, commit)?;
    ensure!(
        git_snapshot(root, commit, None)? == *source,
        "{} build snapshot differs from Git commit {commit}",
        side.name()
    );

    let sources = fingerprint(root, commit, &declarations)?;
    let (entries, coverage) = bind_entries(inventory, sources.entries)?;
    write_report(&Report {
        schema_version: 1,
        scope: "local-executable-inventory-source-inputs",
        side: side.name(),
        commit,
        source_snapshot_sha256: &source.sha256,
        source_plan_sha256: &report.source_plan_sha256,
        fresh_report_sha256: digest(&fresh_bytes),
        build_identity_sha256: &build.sha256,
        inventory_sha256,
        source_declarations_sha256: digest(&declaration_bytes),
        coverage,
        entries,
    })
}

fn validate_build<'a>(
    plan: &Plan,
    plan_bytes: &[u8],
    report: &'a FreshEvidence,
    side: Side,
    inventory_sha256: &str,
    database_bytes: &[u8],
) -> Result<&'a BuildIdentity> {
    ensure!(
        plan.schema_version == 1
            && report.schema_version == 1
            && report.scope == "local-fresh-install-equivalence",
        "unsupported verify-fresh evidence"
    );
    ensure!(
        report.source_plan_sha256 == digest(plan_bytes),
        "verify-fresh report refers to a different source plan"
    );
    ensure!(
        report.reference_source_sha256 == plan.reference.sha256
            && report.candidate_source_sha256 == plan.candidate.sha256,
        "verify-fresh report refers to different source snapshots"
    );
    let build = report
        .build_identity
        .get(side.name())
        .context("selected build identity missing")?;
    ensure!(
        build.sha256 == digest(&serde_json::to_vec(&build.evidence)?),
        "build identity digest differs from its evidence"
    );
    ensure!(
        build.evidence["source_inputs"]["source_sha256"].as_str()
            == Some(side.source(plan).1.sha256.as_str()),
        "build identity refers to a different source snapshot"
    );
    ensure!(
        build.evidence["inventory_sha256"].as_str() == Some(inventory_sha256),
        "executable migration inventory digest differs from build evidence"
    );
    ensure!(
        report.database_report_sha256 == digest(database_bytes),
        "database report digest differs from verify-fresh evidence"
    );
    let database_report =
        serde_json::from_slice(database_bytes).context("invalid database report")?;
    let expected_inputs =
        BTreeMap::from([(format!("{}-plan", side.name()), inventory_sha256.to_owned())]);
    validate_owner_report(&database_report, &expected_inputs)?;
    Ok(build)
}

fn read_inventory(bytes: &[u8]) -> Result<Export> {
    let inventory: Export =
        serde_json::from_slice(bytes).context("invalid executable migration inventory")?;
    ensure!(
        inventory.schema_version == 1
            && !inventory.migrations.is_empty()
            && inventory.entries.len() == inventory.migrations.len(),
        "inventory must contain ordered declarations for every migration"
    );
    let mut seen = BTreeSet::new();
    for (name, declaration) in inventory.migrations.iter().zip(&inventory.entries) {
        ensure!(
            !name.is_empty()
                && name.trim() == name
                && seen.insert(name)
                && declaration["migration"].as_str() == Some(name),
            "inventory contains a repeated or misaligned migration {name:?}"
        );
        ensure!(
            ["owner", "after", "safety", "use_transaction"]
                .iter()
                .all(|field| declaration.get(*field).is_some()),
            "inventory declaration for {name} is incomplete"
        );
    }
    Ok(inventory)
}

fn bind_entries(
    inventory: Export,
    sources: Vec<MigrationInputs>,
) -> Result<(Vec<BoundEntry>, Coverage)> {
    let inventory_steps = inventory.entries.len();
    let declared_source_steps = sources.len();
    let local_link_checked_steps = sources
        .iter()
        .filter(|entry| entry.checked_local_sources.is_some())
        .count();
    let mut by_name: BTreeMap<_, _> = sources
        .into_iter()
        .map(|entry| (entry.migration.clone(), entry))
        .collect();
    let entries = inventory
        .migrations
        .into_iter()
        .zip(inventory.entries)
        .map(|(name, declaration)| BoundEntry {
            declaration,
            source: by_name.remove(&name),
        })
        .collect();
    ensure!(
        by_name.is_empty(),
        "source inputs have no corresponding inventory declaration"
    );
    Ok((
        entries,
        Coverage {
            inventory_steps,
            declared_source_steps,
            local_link_checked_steps,
            transitive_build_inputs_verified: false,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::{snapshot, SourceFile};
    use serde_json::json;

    fn source(path: &str) -> Result<SourceSnapshot> {
        snapshot(vec![SourceFile {
            path: path.into(),
            mode: "100644".into(),
            sha256: digest(b"source"),
        }])
    }

    fn declaration(name: &str) -> Value {
        json!({"migration": name, "owner": {"kind": "module", "slug": "blog"},
            "after": [], "safety": null, "use_transaction": null})
    }

    #[test]
    fn binding_preserves_executable_order_and_marks_uncovered_steps() -> Result<()> {
        let tail = declaration("a");
        let mut first = declaration("z");
        first["use_transaction"] = true.into();
        let inventory = read_inventory(&serde_json::to_vec(&json!({
            "schema_version": 1, "migrations": ["z", "a"], "entries": [first, tail],
        }))?)?;
        let migration_source = MigrationInputs {
            migration: "a".into(),
            source: source("a.rs")?,
            checked_local_sources: Some(BTreeSet::from(["a.rs".into()])),
        };
        let (entries, coverage) = bind_entries(inventory, vec![migration_source])?;
        let entries = serde_json::to_value(entries)?;
        assert_eq!(entries[0]["declaration"], first);
        assert!(entries[0]["source"].is_null());
        assert_eq!(entries[1]["declaration"], tail);
        assert_eq!(
            entries[1]["source"]["checked_local_sources"],
            json!(["a.rs"])
        );
        assert_eq!(coverage.inventory_steps, 2);
        assert_eq!(coverage.declared_source_steps, 1);
        assert_eq!(coverage.local_link_checked_steps, 1);
        assert!(!coverage.transitive_build_inputs_verified);
        Ok(())
    }

    #[test]
    fn rejects_misaligned_or_incomplete_exports() -> Result<()> {
        for export in [
            json!({"migrations":["a"], "entries":[declaration("b")]}),
            json!({"migrations":["a","a"], "entries":[declaration("a"),declaration("a")]}),
            json!({"migrations":["a"], "entries":[]}),
            json!({"migrations":["a"], "entries":[{"migration":"a"}]}),
        ] {
            let mut export = export;
            export["schema_version"] = 1.into();
            assert!(read_inventory(&serde_json::to_vec(&export)?).is_err());
        }
        Ok(())
    }

    #[test]
    fn validates_all_links_to_the_export() -> Result<()> {
        let plan = Plan {
            schema_version: 1,
            purpose: "local migration equivalence evidence".into(),
            reference_commit: "a".repeat(40),
            main_commit: "a".repeat(40),
            candidate_head: "b".repeat(40),
            reference: source("reference.rs")?,
            candidate: source("candidate.rs")?,
            replacement_admission: "blocked".into(),
        };
        let plan_bytes = serde_json::to_vec(&plan)?;
        let inventory_sha256 = digest(b"inventory");
        let database_bytes = serde_json::to_vec(&json!({
            "schema_version": 1,
            "report": {"schema_version": 1, "scope": "fresh-install-stepwise-equivalence", "equivalent": true,
                "replacement_admission": "blocked"},
            "input_sha256": {"reference-plan": inventory_sha256},
        }))?;
        let evidence = json!({"source_inputs": {"source_sha256": plan.reference.sha256}, "inventory_sha256": inventory_sha256});
        let mut report = FreshEvidence {
            schema_version: 1,
            scope: "local-fresh-install-equivalence".into(),
            source_plan_sha256: digest(&plan_bytes),
            reference_source_sha256: plan.reference.sha256.clone(),
            candidate_source_sha256: plan.candidate.sha256.clone(),
            build_identity: BTreeMap::from([(
                "reference".into(),
                BuildIdentity {
                    sha256: digest(&serde_json::to_vec(&evidence)?),
                    evidence,
                },
            )]),
            database_report_sha256: digest(&database_bytes),
        };
        let verify =
            |report: &FreshEvidence, plan_bytes: &[u8], inventory: &str, database: &[u8]| {
                validate_build(
                    &plan,
                    plan_bytes,
                    report,
                    Side::Reference,
                    inventory,
                    database,
                )
                .map(|_| ())
            };
        verify(&report, &plan_bytes, &inventory_sha256, &database_bytes)?;
        assert!(verify(&report, b"other plan", &inventory_sha256, &database_bytes).is_err());
        assert!(verify(
            &report,
            &plan_bytes,
            &digest(b"substituted inventory"),
            &database_bytes
        )
        .is_err());
        assert!(verify(
            &report,
            &plan_bytes,
            &inventory_sha256,
            b"other database report"
        )
        .is_err());
        let unrelated = serde_json::to_vec(&json!({
            "schema_version": 1,
            "report": {"schema_version": 1, "scope": "fresh-install-stepwise-equivalence", "equivalent": true,
                "replacement_admission": "blocked"},
            "input_sha256": {"reference-plan": digest(b"other inventory")},
        }))?;
        report.database_report_sha256 = digest(&unrelated);
        assert!(verify(&report, &plan_bytes, &inventory_sha256, &unrelated).is_err());
        Ok(())
    }
}
