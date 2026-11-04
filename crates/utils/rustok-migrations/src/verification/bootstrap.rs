//! Independent assertions for the eight clock-bearing bootstrap rows on a fresh
//! composed installation. These diagnostics never alter the strict comparison.

use super::{DatabaseSnapshot, Result, TableObservation, VerificationError};
use sea_orm_migration::sea_orm::entity::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};

type Field = (&'static str, &'static str, Option<&'static str>);

struct Seed {
    table: &'static str,
    fields: &'static [Field],
}

const STATE: Field = ("state_id", "text", Some("current"));
const REVISION: Field = ("revision", "bigint", Some("0"));
const GENERATION: Field = ("generation", "bigint", Some("0"));

const SEEDS: &[Seed] = &[
    Seed {
        table: "channel_resolution_invalidation_state",
        fields: &[
            ("scope", "character varying(64)", Some("resolution")),
            GENERATION,
        ],
    },
    Seed {
        table: "flex_field_definition_cache_generation",
        fields: &[("id", "integer", Some("1")), GENERATION],
    },
    Seed {
        table: "module_artifact_node_reconciliation_state",
        fields: &[
            STATE,
            REVISION,
            ("desired_reconciliation_id", "uuid", None),
            ("observed_reconciliation_id", "uuid", None),
        ],
    },
    Seed {
        table: "module_security_epoch",
        fields: &[
            ("singleton_id", "integer", Some("1")),
            ("epoch", "bigint", Some("1")),
            (
                "last_reason",
                "text",
                Some("Initial security epoch bootstrap"),
            ),
        ],
    },
    Seed {
        table: "module_static_distribution_release_state",
        fields: &[STATE, REVISION, ("active_release_id", "uuid", None)],
    },
    Seed {
        table: "module_static_distribution_rollout_state",
        fields: &[
            STATE,
            REVISION,
            ("desired_rollout_id", "uuid", None),
            ("observed_rollout_id", "uuid", None),
        ],
    },
    Seed {
        table: "module_static_distribution_state",
        fields: &[STATE, REVISION, ("current_build_id", "uuid", None)],
    },
    Seed {
        table: "rbac_invalidation_state",
        fields: &[
            ("scope", "character varying(64)", Some("permissions")),
            GENERATION,
        ],
    },
];

pub struct InstallWindow {
    start: DateTimeWithTimeZone,
    end: DateTimeWithTimeZone,
}

impl InstallWindow {
    pub fn new(start: DateTimeWithTimeZone, end: DateTimeWithTimeZone) -> Result<Self> {
        if start > end {
            return Err(VerificationError::Incomplete(
                "PostgreSQL clock moved backwards during installation".into(),
            ));
        }
        Ok(Self { start, end })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapCheck {
    MissingTable,
    UnexpectedColumns,
    UnexpectedRowCount,
    UnexpectedValues,
    InvalidTimestamp,
    TimestampOutsideInstall,
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootstrapTable {
    pub table: String,
    pub reference: BootstrapCheck,
    pub candidate: BootstrapCheck,
    pub clock_only_difference: bool,
}

pub fn diagnose(
    reference: (&DatabaseSnapshot, &InstallWindow),
    candidate: (&DatabaseSnapshot, &InstallWindow),
) -> Vec<BootstrapTable> {
    SEEDS
        .iter()
        .map(|seed| {
            let original = find_table(reference.0, seed);
            let proposed = find_table(candidate.0, seed);
            let original_check = check(original, seed, reference.1);
            let proposed_check = check(proposed, seed, candidate.1);
            // Two verified observations have the same columns and expected values
            // in every position except the final timestamp.
            let clock_only_difference = original_check == BootstrapCheck::Verified
                && proposed_check == BootstrapCheck::Verified
                && original != proposed;
            BootstrapTable {
                table: seed.table.to_owned(),
                reference: original_check,
                candidate: proposed_check,
                clock_only_difference,
            }
        })
        .collect()
}

fn find_table<'a>(snapshot: &'a DatabaseSnapshot, seed: &Seed) -> Option<&'a TableObservation> {
    snapshot
        .tables
        .iter()
        .find(|table| table.relation.schema == "public" && table.relation.name == seed.table)
}

fn check(table: Option<&TableObservation>, seed: &Seed, window: &InstallWindow) -> BootstrapCheck {
    let Some(table) = table else {
        return BootstrapCheck::MissingTable;
    };
    let expected_columns = seed
        .fields
        .iter()
        .map(|(name, ty, _)| serde_json::json!([name, ty]).to_string())
        .chain([serde_json::json!(["updated_at", "timestamp with time zone"]).to_string()]);
    if !table
        .columns
        .iter()
        .map(String::as_str)
        .eq(expected_columns)
    {
        return BootstrapCheck::UnexpectedColumns;
    }
    let [row] = table.rows.as_slice() else {
        return BootstrapCheck::UnexpectedRowCount;
    };
    let Ok(values) = serde_json::from_str::<Vec<Option<String>>>(row) else {
        return BootstrapCheck::UnexpectedValues;
    };
    let Some((timestamp, fields)) = values.split_last() else {
        return BootstrapCheck::UnexpectedValues;
    };
    if !fields
        .iter()
        .map(Option::as_deref)
        .eq(seed.fields.iter().map(|(_, _, value)| *value))
    {
        return BootstrapCheck::UnexpectedValues;
    }
    // The observer uses ISO/YMD, UTC and PostgreSQL's text output for timestamptz.
    let Some(timestamp) = timestamp.as_deref().and_then(|value| {
        DateTimeWithTimeZone::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f%#z").ok()
    }) else {
        return BootstrapCheck::InvalidTimestamp;
    };
    if timestamp < window.start || timestamp > window.end {
        return BootstrapCheck::TimestampOutsideInstall;
    }
    BootstrapCheck::Verified
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::RelationName;

    #[test]
    fn clock_diagnostic_requires_an_exact_singleton_inside_the_window() {
        let window = InstallWindow::new(
            "2026-11-04T15:00:00Z".parse().unwrap(),
            "2026-11-04T15:00:01Z".parse().unwrap(),
        )
        .unwrap();
        let seed = &SEEDS[6];
        let mut table = TableObservation {
            relation: RelationName {
                schema: "public".into(),
                name: seed.table.into(),
            },
            columns: [
                r#"["state_id","text"]"#,
                r#"["revision","bigint"]"#,
                r#"["current_build_id","uuid"]"#,
                r#"["updated_at","timestamp with time zone"]"#,
            ]
            .map(String::from)
            .into(),
            rows: vec![r#"["current", "0", null, "2026-11-04 15:00:00.000001+00"]"#.into()],
        };
        assert_eq!(check(Some(&table), seed, &window), BootstrapCheck::Verified);
        table.rows.push(table.rows[0].clone());
        assert_eq!(
            check(Some(&table), seed, &window),
            BootstrapCheck::UnexpectedRowCount
        );
        table.rows = vec![r#"["current", "1", null, "2026-11-04 15:00:00.000001+00"]"#.into()];
        assert_eq!(
            check(Some(&table), seed, &window),
            BootstrapCheck::UnexpectedValues
        );
        table.rows = vec![
            r#"["current", "0", "00000000-0000-0000-0000-000000000001", "2026-11-04 15:00:00+00"]"#
                .into(),
        ];
        assert_eq!(
            check(Some(&table), seed, &window),
            BootstrapCheck::UnexpectedValues
        );
        table.rows = vec![r#"["current", "0", null, "2026-11-04 15:00:01.000001+00"]"#.into()];
        assert_eq!(
            check(Some(&table), seed, &window),
            BootstrapCheck::TimestampOutsideInstall
        );
        table.rows = vec![r#"["current", "0", null, "infinity"]"#.into()];
        assert_eq!(
            check(Some(&table), seed, &window),
            BootstrapCheck::InvalidTimestamp
        );
        table.columns[3] = r#"["updated_at","text"]"#.into();
        assert_eq!(
            check(Some(&table), seed, &window),
            BootstrapCheck::UnexpectedColumns
        );
    }
}
