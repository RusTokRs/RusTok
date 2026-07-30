use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexValueType {
    Boolean,
    Integer,
    Decimal,
    String,
    Uuid,
    Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum IndexValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Decimal(Decimal),
    String(String),
    Uuid(Uuid),
    Timestamp(DateTime<Utc>),
    List(Vec<IndexValue>),
}

impl IndexValue {
    pub fn value_type(&self) -> Option<IndexValueType> {
        match self {
            Self::Null => None,
            Self::Boolean(_) => Some(IndexValueType::Boolean),
            Self::Integer(_) => Some(IndexValueType::Integer),
            Self::Decimal(_) => Some(IndexValueType::Decimal),
            Self::String(_) => Some(IndexValueType::String),
            Self::Uuid(_) => Some(IndexValueType::Uuid),
            Self::Timestamp(_) => Some(IndexValueType::Timestamp),
            Self::List(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use serde_json::json;

    use super::IndexValue;

    #[test]
    fn decimal_tagged_json_uses_exact_string_wire() {
        let value = IndexValue::Decimal(Decimal::new(1_234_500, 4));
        let encoded = serde_json::to_value(&value).unwrap();

        assert_eq!(
            encoded,
            json!({
                "type": "decimal",
                "value": "123.4500"
            })
        );

        let decoded: IndexValue = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), encoded);
    }
}
