use std::{env, path::PathBuf};

use anyhow::{Context, Result, bail};
use rustok_index::LocaleKey;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetScale {
    Smoke,
    Rows100k,
    Rows1m,
}

impl DatasetScale {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "smoke" => Ok(Self::Smoke),
            "100k" => Ok(Self::Rows100k),
            "1m" => Ok(Self::Rows1m),
            other => bail!("INDEX_BENCH_SCALE must be smoke, 100k, or 1m; got {other}"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DatasetConfig {
    pub scale: DatasetScale,
    pub tenants: u32,
    pub products_per_tenant: u32,
    pub locales: Vec<String>,
    pub variants_per_product: u32,
    pub channels_per_tenant: u32,
}

impl DatasetConfig {
    pub fn for_scale(scale: DatasetScale, locales: Vec<String>) -> Result<Self> {
        let locale_count = u32::try_from(locales.len()).context("too many locales")?;
        if locale_count == 0 {
            bail!("at least one locale is required");
        }

        let (tenants, product_rows) = match scale {
            DatasetScale::Smoke => (2, 400),
            DatasetScale::Rows100k => (10, 100_000),
            DatasetScale::Rows1m => (20, 1_000_000),
        };
        let denominator = tenants * locale_count;
        if product_rows % denominator != 0 {
            bail!("scale rows must divide evenly across tenants and locales");
        }

        Ok(Self {
            scale,
            tenants,
            products_per_tenant: product_rows / denominator,
            locales,
            variants_per_product: 2,
            channels_per_tenant: 8,
        })
    }

    pub fn product_rows(&self) -> u64 {
        u64::from(self.tenants)
            * u64::from(self.products_per_tenant)
            * self.locales.len() as u64
    }

    pub fn variant_rows(&self) -> u64 {
        self.product_rows() * u64::from(self.variants_per_product)
    }

    pub fn channel_rows(&self) -> u64 {
        u64::from(self.tenants) * u64::from(self.channels_per_tenant)
    }

    pub fn total_entity_rows(&self) -> u64 {
        self.product_rows() + self.variant_rows() + self.channel_rows()
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub database_url: String,
    pub dataset: DatasetConfig,
    pub repetitions: u32,
    pub output_path: PathBuf,
}

impl BenchmarkConfig {
    pub fn from_env() -> Result<Self> {
        let database_url = env::var("DATABASE_URL")
            .context("DATABASE_URL is required for index-storage-benchmark")?;
        let scale = DatasetScale::parse(
            &env::var("INDEX_BENCH_SCALE").unwrap_or_else(|_| "smoke".to_owned()),
        )?;
        let locales = parse_locales(
            &env::var("INDEX_BENCH_LOCALES").unwrap_or_else(|_| "en-US,ru-RU".to_owned()),
        )?;
        let repetitions = env::var("INDEX_BENCH_REPETITIONS")
            .unwrap_or_else(|_| "3".to_owned())
            .parse::<u32>()
            .context("INDEX_BENCH_REPETITIONS must be an integer")?;
        if repetitions == 0 {
            bail!("INDEX_BENCH_REPETITIONS must be greater than zero");
        }
        let output_path = env::var("INDEX_BENCH_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target/index-storage-benchmark/report.json"));

        Ok(Self {
            database_url,
            dataset: DatasetConfig::for_scale(scale, locales)?,
            repetitions,
            output_path,
        })
    }
}

fn parse_locales(raw: &str) -> Result<Vec<String>> {
    let mut locales = Vec::new();
    for value in raw.split(',').map(str::trim).filter(|value| !value.is_empty()) {
        let locale = LocaleKey::new(value)
            .with_context(|| format!("invalid benchmark locale: {value}"))?
            .into_inner();
        if !locales.contains(&locale) {
            locales.push(locale);
        }
    }
    if locales.is_empty() {
        bail!("INDEX_BENCH_LOCALES must contain at least one locale");
    }
    Ok(locales)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_presets_have_exact_product_row_counts() {
        let locales = vec!["en-US".to_owned(), "ru-RU".to_owned()];
        assert_eq!(
            DatasetConfig::for_scale(DatasetScale::Rows100k, locales.clone())
                .unwrap()
                .product_rows(),
            100_000
        );
        assert_eq!(
            DatasetConfig::for_scale(DatasetScale::Rows1m, locales)
                .unwrap()
                .product_rows(),
            1_000_000
        );
    }
}
