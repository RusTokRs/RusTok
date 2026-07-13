use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE regions
                            ADD CONSTRAINT ck_regions_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_regions_tax_rate
                            CHECK (tax_rate >= 0 AND tax_rate <= 100) NOT VALID,
                            ADD CONSTRAINT ck_regions_tax_provider
                            CHECK (
                                tax_provider_id IS NULL
                                OR tax_provider_id ~ '^[a-z0-9_-]{1,64}$'
                            ) NOT VALID,
                            ADD CONSTRAINT ck_regions_countries_array
                            CHECK (jsonb_typeof(countries) = 'array') NOT VALID;

                        ALTER TABLE region_country_tax_policies
                            ADD CONSTRAINT ck_region_country_tax_policies_country
                            CHECK (country_code ~ '^[A-Z]{2}$') NOT VALID,
                            ADD CONSTRAINT ck_region_country_tax_policies_rate
                            CHECK (tax_rate >= 0 AND tax_rate <= 100) NOT VALID;

                        CREATE OR REPLACE FUNCTION enforce_region_country_integrity()
                        RETURNS trigger AS $$
                        BEGIN
                            IF jsonb_typeof(NEW.countries) <> 'array' THEN
                                RAISE EXCEPTION 'region countries must be a JSON array'
                                    USING ERRCODE = '23514';
                            END IF;
                            IF EXISTS (
                                SELECT 1 FROM jsonb_array_elements_text(NEW.countries) country
                                WHERE country !~ '^[A-Z]{2}$'
                            ) THEN
                                RAISE EXCEPTION 'region countries must contain uppercase 2-letter codes'
                                    USING ERRCODE = '23514';
                            END IF;
                            IF EXISTS (
                                SELECT country
                                FROM jsonb_array_elements_text(NEW.countries) country
                                GROUP BY country HAVING count(*) > 1
                            ) THEN
                                RAISE EXCEPTION 'region countries contain duplicates'
                                    USING ERRCODE = '23514';
                            END IF;
                            IF EXISTS (
                                SELECT 1
                                FROM regions other
                                CROSS JOIN LATERAL jsonb_array_elements_text(other.countries) existing_country
                                JOIN LATERAL jsonb_array_elements_text(NEW.countries) incoming_country
                                  ON incoming_country = existing_country
                                WHERE other.tenant_id = NEW.tenant_id
                                  AND other.id <> NEW.id
                            ) THEN
                                RAISE EXCEPTION 'a country may belong to only one region per tenant'
                                    USING ERRCODE = '23505';
                            END IF;
                            IF TG_OP = 'UPDATE' AND EXISTS (
                                SELECT 1
                                FROM region_country_tax_policies policy
                                WHERE policy.region_id = NEW.id
                                  AND NOT (NEW.countries ? policy.country_code)
                            ) THEN
                                RAISE EXCEPTION 'region countries cannot exclude an existing country tax policy'
                                    USING ERRCODE = '23514';
                            END IF;
                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER regions_country_integrity_guard
                        BEFORE INSERT OR UPDATE OF tenant_id, countries
                        ON regions FOR EACH ROW
                        EXECUTE FUNCTION enforce_region_country_integrity();

                        CREATE OR REPLACE FUNCTION enforce_region_country_policy_integrity()
                        RETURNS trigger AS $$
                        BEGIN
                            IF NOT EXISTS (
                                SELECT 1 FROM regions region
                                WHERE region.id = NEW.region_id
                                  AND region.countries ? NEW.country_code
                            ) THEN
                                RAISE EXCEPTION 'country tax policy must target a country owned by the region'
                                    USING ERRCODE = '23514';
                            END IF;
                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER region_country_tax_policies_integrity_guard
                        BEFORE INSERT OR UPDATE OF region_id, country_code
                        ON region_country_tax_policies FOR EACH ROW
                        EXECUTE FUNCTION enforce_region_country_policy_integrity();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER regions_policy_guard_insert
                        BEFORE INSERT ON regions FOR EACH ROW BEGIN
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid region currency') END;
                            SELECT CASE WHEN NEW.tax_rate < 0 OR NEW.tax_rate > 100
                                THEN RAISE(ABORT, 'invalid region tax rate') END;
                            SELECT CASE WHEN NEW.tax_provider_id IS NOT NULL AND (
                                length(trim(NEW.tax_provider_id)) = 0 OR length(NEW.tax_provider_id) > 64
                                OR NEW.tax_provider_id GLOB '*[^a-z0-9_-]*'
                            ) THEN RAISE(ABORT, 'invalid region tax provider') END;
                            SELECT CASE WHEN json_valid(NEW.countries) = 0 OR json_type(NEW.countries) <> 'array'
                                THEN RAISE(ABORT, 'region countries must be a JSON array') END;
                            SELECT CASE WHEN EXISTS (
                                SELECT 1 FROM json_each(NEW.countries)
                                WHERE length(value) <> 2 OR value GLOB '*[^A-Z]*'
                            ) THEN RAISE(ABORT, 'invalid region country code') END;
                            SELECT CASE WHEN EXISTS (
                                SELECT value FROM json_each(NEW.countries)
                                GROUP BY value HAVING count(*) > 1
                            ) THEN RAISE(ABORT, 'duplicate region country code') END;
                            SELECT CASE WHEN EXISTS (
                                SELECT 1
                                FROM regions other
                                JOIN json_each(other.countries) existing_country
                                JOIN json_each(NEW.countries) incoming_country
                                  ON incoming_country.value = existing_country.value
                                WHERE other.tenant_id = NEW.tenant_id AND other.id <> NEW.id
                            ) THEN RAISE(ABORT, 'country belongs to another tenant region') END;
                        END;

                        CREATE TRIGGER regions_policy_guard_update
                        BEFORE UPDATE OF tenant_id, currency_code, tax_rate, tax_provider_id, countries
                        ON regions FOR EACH ROW BEGIN
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid region currency') END;
                            SELECT CASE WHEN NEW.tax_rate < 0 OR NEW.tax_rate > 100
                                THEN RAISE(ABORT, 'invalid region tax rate') END;
                            SELECT CASE WHEN NEW.tax_provider_id IS NOT NULL AND (
                                length(trim(NEW.tax_provider_id)) = 0 OR length(NEW.tax_provider_id) > 64
                                OR NEW.tax_provider_id GLOB '*[^a-z0-9_-]*'
                            ) THEN RAISE(ABORT, 'invalid region tax provider') END;
                            SELECT CASE WHEN json_valid(NEW.countries) = 0 OR json_type(NEW.countries) <> 'array'
                                THEN RAISE(ABORT, 'region countries must be a JSON array') END;
                            SELECT CASE WHEN EXISTS (
                                SELECT 1 FROM json_each(NEW.countries)
                                WHERE length(value) <> 2 OR value GLOB '*[^A-Z]*'
                            ) THEN RAISE(ABORT, 'invalid region country code') END;
                            SELECT CASE WHEN EXISTS (
                                SELECT value FROM json_each(NEW.countries)
                                GROUP BY value HAVING count(*) > 1
                            ) THEN RAISE(ABORT, 'duplicate region country code') END;
                            SELECT CASE WHEN EXISTS (
                                SELECT 1
                                FROM regions other
                                JOIN json_each(other.countries) existing_country
                                JOIN json_each(NEW.countries) incoming_country
                                  ON incoming_country.value = existing_country.value
                                WHERE other.tenant_id = NEW.tenant_id AND other.id <> NEW.id
                            ) THEN RAISE(ABORT, 'country belongs to another tenant region') END;
                            SELECT CASE WHEN EXISTS (
                                SELECT 1 FROM region_country_tax_policies policy
                                WHERE policy.region_id = NEW.id
                                  AND NOT EXISTS (
                                      SELECT 1 FROM json_each(NEW.countries)
                                      WHERE value = policy.country_code
                                  )
                            ) THEN RAISE(ABORT, 'region excludes an existing country tax policy') END;
                        END;

                        CREATE TRIGGER region_country_tax_policies_guard_insert
                        BEFORE INSERT ON region_country_tax_policies FOR EACH ROW BEGIN
                            SELECT CASE WHEN length(NEW.country_code) <> 2 OR NEW.country_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid country tax policy country') END;
                            SELECT CASE WHEN NEW.tax_rate < 0 OR NEW.tax_rate > 100
                                THEN RAISE(ABORT, 'invalid country tax policy rate') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM regions region, json_each(region.countries) country
                                WHERE region.id = NEW.region_id AND country.value = NEW.country_code
                            ) THEN RAISE(ABORT, 'country tax policy is outside region countries') END;
                        END;

                        CREATE TRIGGER region_country_tax_policies_guard_update
                        BEFORE UPDATE OF region_id, country_code, tax_rate
                        ON region_country_tax_policies FOR EACH ROW BEGIN
                            SELECT CASE WHEN length(NEW.country_code) <> 2 OR NEW.country_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid country tax policy country') END;
                            SELECT CASE WHEN NEW.tax_rate < 0 OR NEW.tax_rate > 100
                                THEN RAISE(ABORT, 'invalid country tax policy rate') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM regions region, json_each(region.countries) country
                                WHERE region.id = NEW.region_id AND country.value = NEW.country_code
                            ) THEN RAISE(ABORT, 'country tax policy is outside region countries') END;
                        END;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS region_country_tax_policies_integrity_guard ON region_country_tax_policies;
                        DROP FUNCTION IF EXISTS enforce_region_country_policy_integrity();
                        DROP TRIGGER IF EXISTS regions_country_integrity_guard ON regions;
                        DROP FUNCTION IF EXISTS enforce_region_country_integrity();
                        ALTER TABLE region_country_tax_policies
                            DROP CONSTRAINT IF EXISTS ck_region_country_tax_policies_rate,
                            DROP CONSTRAINT IF EXISTS ck_region_country_tax_policies_country;
                        ALTER TABLE regions
                            DROP CONSTRAINT IF EXISTS ck_regions_countries_array,
                            DROP CONSTRAINT IF EXISTS ck_regions_tax_provider,
                            DROP CONSTRAINT IF EXISTS ck_regions_tax_rate,
                            DROP CONSTRAINT IF EXISTS ck_regions_currency;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS region_country_tax_policies_guard_update;
                        DROP TRIGGER IF EXISTS region_country_tax_policies_guard_insert;
                        DROP TRIGGER IF EXISTS regions_policy_guard_update;
                        DROP TRIGGER IF EXISTS regions_policy_guard_insert;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }
}
