mod common;
mod eav;
mod hot;
mod jsonb;
mod source;

use serde::Serialize;

use super::DatasetConfig;

pub const SOURCE_SCHEMA: &str = "idx_bench_source";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Prototype {
    Jsonb,
    TypedEav,
    HotProjection,
}

impl Prototype {
    pub const ALL: [Self; 3] = [Self::Jsonb, Self::TypedEav, Self::HotProjection];

    pub const fn schema(self) -> &'static str {
        match self {
            Self::Jsonb => "idx_bench_jsonb",
            Self::TypedEav => "idx_bench_eav",
            Self::HotProjection => "idx_bench_hot",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Workload {
    pub name: &'static str,
    pub sql: String,
}

pub fn source_dataset_sql(config: &DatasetConfig) -> String {
    source::dataset_sql(config)
}

pub fn prototype_sql(prototype: Prototype) -> String {
    match prototype {
        Prototype::Jsonb => jsonb::prototype_sql(),
        Prototype::TypedEav => eav::prototype_sql(),
        Prototype::HotProjection => hot::prototype_sql(),
    }
}

pub fn full_prototype_sql(prototype: Prototype) -> String {
    let mut sql = prototype_sql(prototype);
    sql.push_str(&common::link_sql(prototype.schema()));
    sql.push_str(match prototype {
        Prototype::Jsonb => {
            "\nANALYZE idx_bench_jsonb.entity;\nANALYZE idx_bench_jsonb.link;\n"
        }
        Prototype::TypedEav => {
            "\nANALYZE idx_bench_eav.entity;\nANALYZE idx_bench_eav.field_value;\nANALYZE idx_bench_eav.link;\n"
        }
        Prototype::HotProjection => {
            "\nANALYZE idx_bench_hot.product;\nANALYZE idx_bench_hot.variant;\nANALYZE idx_bench_hot.sales_channel;\nANALYZE idx_bench_hot.link;\n"
        }
    });
    sql
}

pub fn workloads(prototype: Prototype, config: &DatasetConfig) -> Vec<Workload> {
    let context = WorkloadContext::new(config);
    match prototype {
        Prototype::Jsonb => jsonb::workloads(&context),
        Prototype::TypedEav => eav::workloads(&context),
        Prototype::HotProjection => hot::workloads(&context),
    }
}

pub(super) struct WorkloadContext {
    pub tenant: &'static str,
    pub locale: String,
    pub anchor_price: i64,
    pub anchor_id: String,
}

impl WorkloadContext {
    fn new(config: &DatasetConfig) -> Self {
        let anchor_no = (config.products_per_tenant / 2).max(1);
        Self {
            tenant: "md5('tenant:1')::uuid",
            locale: sql_literal(&config.locales[0]),
            anchor_price: 500 + ((i64::from(anchor_no) * 7919 + 101) % 200000),
            anchor_id: format!("md5('product:1:{anchor_no}')::uuid"),
        }
    }
}

pub(super) fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_storage::{DatasetConfig, DatasetScale};

    #[test]
    fn generated_sql_is_deterministic_and_separates_links() {
        let config = DatasetConfig::for_scale(
            DatasetScale::Smoke,
            vec!["en-US".to_owned(), "ru-RU".to_owned()],
        )
        .unwrap();
        assert_eq!(source_dataset_sql(&config), source_dataset_sql(&config));
        for prototype in Prototype::ALL {
            let sql = full_prototype_sql(prototype);
            assert!(sql.contains(".link"));
            assert!(sql.contains("source_entity"));
            assert!(sql.contains("target_entity"));
            assert!(!sql.contains(&format!("ANALYZE {};", prototype.schema())));
        }
    }

    #[test]
    fn every_candidate_exposes_the_same_workload_names() {
        let config = DatasetConfig::for_scale(
            DatasetScale::Smoke,
            vec!["en-US".to_owned(), "ru-RU".to_owned()],
        )
        .unwrap();
        let expected = workloads(Prototype::Jsonb, &config)
            .into_iter()
            .map(|workload| workload.name)
            .collect::<Vec<_>>();
        for prototype in [Prototype::TypedEav, Prototype::HotProjection] {
            assert_eq!(
                workloads(prototype, &config)
                    .into_iter()
                    .map(|workload| workload.name)
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
}
