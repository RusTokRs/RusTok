mod common;
mod eav;
mod hot;
mod jsonb;
mod maintenance;
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

    pub const fn relations(self) -> &'static [&'static str] {
        match self {
            Self::Jsonb => &["entity", "link"],
            Self::TypedEav => &["entity", "field_value", "link"],
            Self::HotProjection => &["product", "variant", "sales_channel", "link"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Workload {
    pub name: &'static str,
    pub sql: String,
}

#[derive(Debug, Clone)]
pub struct MutationWorkload {
    pub name: &'static str,
    pub sql: String,
    pub expected_affected_entities: i64,
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
    sql.push_str(&analyze_sql(prototype));
    sql
}

pub fn workloads(prototype: Prototype, config: &DatasetConfig) -> Vec<Workload> {
    let context = WorkloadContext::new(config);
    let workloads = match prototype {
        Prototype::Jsonb => jsonb::workloads(&context),
        Prototype::TypedEav => eav::workloads(&context),
        Prototype::HotProjection => hot::workloads(&context),
    };
    workloads
        .into_iter()
        .map(|mut workload| {
            workload.sql = common::assert_full_link_identity_sql(workload.sql);
            workload
        })
        .collect()
}

pub fn mutation_workloads(
    prototype: Prototype,
    config: &DatasetConfig,
) -> Vec<MutationWorkload> {
    let context = WorkloadContext::new(config);
    let workloads = match prototype {
        Prototype::Jsonb => jsonb::mutation_workloads(&context),
        Prototype::TypedEav => eav::mutation_workloads(&context),
        Prototype::HotProjection => hot::mutation_workloads(&context),
    };
    workloads
        .into_iter()
        .map(|mut workload| {
            workload.sql = common::assert_full_link_identity_sql(workload.sql);
            workload
        })
        .collect()
}

pub fn churn_cycle_sql(prototype: Prototype, config: &DatasetConfig) -> String {
    common::assert_full_link_identity_sql(maintenance::churn_cycle_sql(
        prototype,
        &WorkloadContext::new(config),
    ))
}

pub fn analyze_sql(prototype: Prototype) -> String {
    prototype
        .relations()
        .iter()
        .map(|relation| format!("ANALYZE {}.{relation};", prototype.schema()))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

pub fn vacuum_statements(prototype: Prototype) -> Vec<String> {
    prototype
        .relations()
        .iter()
        .map(|relation| format!("VACUUM (ANALYZE) {}.{relation};", prototype.schema()))
        .collect()
}

pub(super) struct WorkloadContext {
    pub tenant: &'static str,
    pub locale: String,
    pub anchor_price: i64,
    pub anchor_id: String,
    pub mutation_batch: u32,
    pub churn_first_product: u32,
    pub variants_per_product: u32,
}

impl WorkloadContext {
    fn new(config: &DatasetConfig) -> Self {
        let anchor_no = (config.products_per_tenant / 2).max(1);
        let mutation_batch = config.products_per_tenant.min(1_000).max(1);
        Self {
            tenant: "md5('tenant:1')::uuid",
            locale: sql_literal(&config.locales[0]),
            anchor_price: 500 + ((i64::from(anchor_no) * 7919 + 101) % 200000),
            anchor_id: format!("md5('product:1:{anchor_no}')::uuid"),
            mutation_batch,
            churn_first_product: config.products_per_tenant - mutation_batch + 1,
            variants_per_product: config.variants_per_product,
        }
    }

    pub fn expected_deleted_links(&self) -> u64 {
        u64::from(self.mutation_batch) * u64::from(self.variants_per_product)
    }
}

pub(super) fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_storage::{DatasetConfig, DatasetScale};

    fn smoke_config() -> DatasetConfig {
        DatasetConfig::for_scale(
            DatasetScale::Smoke,
            vec!["en-US".to_owned(), "ru-RU".to_owned()],
        )
        .unwrap()
    }

    #[test]
    fn generated_sql_is_deterministic_and_separates_links() {
        let config = smoke_config();
        assert_eq!(source_dataset_sql(&config), source_dataset_sql(&config));
        for prototype in Prototype::ALL {
            let sql = full_prototype_sql(prototype);
            assert!(sql.contains(".link"));
            assert!(sql.contains("source_entity"));
            assert!(sql.contains("target_entity"));
            assert!(!sql.contains(&format!("ANALYZE {};", prototype.schema())));
            assert!(churn_cycle_sql(prototype, &config).contains("DELETE FROM"));
            let vacuum = vacuum_statements(prototype);
            assert_eq!(vacuum.len(), prototype.relations().len());
            assert!(vacuum.iter().all(|sql| sql.starts_with("VACUUM (ANALYZE)")));
        }
    }

    #[test]
    fn every_candidate_exposes_the_same_read_and_mutation_names() {
        let config = smoke_config();
        let expected_reads = workloads(Prototype::Jsonb, &config)
            .into_iter()
            .map(|workload| workload.name)
            .collect::<Vec<_>>();
        let expected_mutations = mutation_workloads(Prototype::Jsonb, &config)
            .into_iter()
            .map(|workload| workload.name)
            .collect::<Vec<_>>();
        for prototype in [Prototype::TypedEav, Prototype::HotProjection] {
            assert_eq!(
                workloads(prototype, &config)
                    .into_iter()
                    .map(|workload| workload.name)
                    .collect::<Vec<_>>(),
                expected_reads
            );
            assert_eq!(
                mutation_workloads(prototype, &config)
                    .into_iter()
                    .map(|workload| workload.name)
                    .collect::<Vec<_>>(),
                expected_mutations
            );
        }
    }

    #[test]
    fn churn_batch_uses_the_tail_of_large_datasets() {
        let config = DatasetConfig::for_scale(
            DatasetScale::Rows100k,
            vec!["en-US".to_owned(), "ru-RU".to_owned()],
        )
        .unwrap();
        let context = WorkloadContext::new(&config);
        assert_eq!(context.mutation_batch, 1_000);
        assert_eq!(context.churn_first_product, 4_001);
    }
}
