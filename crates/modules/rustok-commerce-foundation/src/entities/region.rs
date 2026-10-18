use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "regions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub currency_code: String,
    pub tax_provider_id: Option<String>,
    pub tax_rate: Decimal,
    pub tax_included: bool,
    pub countries: Json,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Model {
    /// Calculates tax amount on a net taxable amount.
    pub fn calculate_tax_amount(&self, taxable_amount: Decimal) -> Decimal {
        if self.tax_rate <= Decimal::ZERO || taxable_amount <= Decimal::ZERO {
            Decimal::ZERO
        } else {
            (taxable_amount * self.tax_rate) / Decimal::from(100)
        }
    }

    /// Extracts tax portion from a gross amount when tax is included in price.
    pub fn extract_included_tax(&self, gross_amount: Decimal) -> Decimal {
        if self.tax_rate <= Decimal::ZERO || gross_amount <= Decimal::ZERO || !self.tax_included {
            Decimal::ZERO
        } else {
            let divisor = Decimal::ONE + (self.tax_rate / Decimal::from(100));
            gross_amount - (gross_amount / divisor)
        }
    }

    /// Checks if region configuration supports the given 2-letter ISO country code.
    pub fn supports_country(&self, country_code: &str) -> bool {
        let code = country_code.trim();
        if let Some(list) = self.countries.as_array() {
            list.iter().any(|val| {
                val.as_str()
                    .is_some_and(|c| c.eq_ignore_ascii_case(code))
            })
        } else {
            false
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_region_tax_and_country_helpers() {
        let region = Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            currency_code: "RUB".into(),
            tax_provider_id: None,
            tax_rate: Decimal::new(20, 0), // 20%
            tax_included: true,
            countries: serde_json::json!(["RU", "BY", "KZ"]),
            metadata: serde_json::json!({}),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        };

        // 20% on 100 net = 20
        assert_eq!(
            region.calculate_tax_amount(Decimal::new(100, 0)),
            Decimal::new(20, 0)
        );

        // 20% included in 120 gross = 20
        let included_tax = region.extract_included_tax(Decimal::new(120, 0));
        assert_eq!(included_tax, Decimal::new(20, 0));

        assert!(region.supports_country("RU"));
        assert!(region.supports_country("ru"));
        assert!(region.supports_country("by"));
        assert!(!region.supports_country("US"));
    }
}
