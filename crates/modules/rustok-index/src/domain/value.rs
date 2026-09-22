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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum TaggedIndexValue<'a> {
    Null,
    Boolean(bool),
    Integer(i64),
    Decimal(Decimal),
    String(&'a str),
    Uuid(Uuid),
    Timestamp(DateTime<Utc>),
    List(Vec<IndexValue>),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum TaggedIndexValueOwned {
    Null,
    Boolean(bool),
    Integer(i64),
    Decimal(Decimal),
    String(String),
    Uuid(Uuid),
    Timestamp(DateTime<Utc>),
    List(Vec<IndexValue>),
}

#[derive(Serialize, Deserialize)]
enum BinaryIndexValue<'a> {
    Null,
    Boolean(bool),
    Integer(i64),
    Decimal(Decimal),
    String(&'a str),
    Uuid(Uuid),
    Timestamp(DateTime<Utc>),
    List(Vec<IndexValue>),
}

#[derive(Serialize, Deserialize)]
enum BinaryIndexValueOwned {
    Null,
    Boolean(bool),
    Integer(i64),
    Decimal(Decimal),
    String(String),
    Uuid(Uuid),
    Timestamp(DateTime<Utc>),
    List(Vec<IndexValue>),
}

impl Serialize for IndexValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            let tagged = match self {
                Self::Null => TaggedIndexValue::Null,
                Self::Boolean(v) => TaggedIndexValue::Boolean(*v),
                Self::Integer(v) => TaggedIndexValue::Integer(*v),
                Self::Decimal(v) => TaggedIndexValue::Decimal(*v),
                Self::String(v) => TaggedIndexValue::String(v.as_str()),
                Self::Uuid(v) => TaggedIndexValue::Uuid(*v),
                Self::Timestamp(v) => TaggedIndexValue::Timestamp(*v),
                Self::List(v) => TaggedIndexValue::List(v.clone()),
            };
            tagged.serialize(serializer)
        } else {
            let binary = match self {
                Self::Null => BinaryIndexValue::Null,
                Self::Boolean(v) => BinaryIndexValue::Boolean(*v),
                Self::Integer(v) => BinaryIndexValue::Integer(*v),
                Self::Decimal(v) => BinaryIndexValue::Decimal(*v),
                Self::String(v) => BinaryIndexValue::String(v.as_str()),
                Self::Uuid(v) => BinaryIndexValue::Uuid(*v),
                Self::Timestamp(v) => BinaryIndexValue::Timestamp(*v),
                Self::List(v) => BinaryIndexValue::List(v.clone()),
            };
            binary.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for IndexValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let tagged = TaggedIndexValueOwned::deserialize(deserializer)?;
            Ok(match tagged {
                TaggedIndexValueOwned::Null => Self::Null,
                TaggedIndexValueOwned::Boolean(v) => Self::Boolean(v),
                TaggedIndexValueOwned::Integer(v) => Self::Integer(v),
                TaggedIndexValueOwned::Decimal(v) => Self::Decimal(v),
                TaggedIndexValueOwned::String(v) => Self::String(v),
                TaggedIndexValueOwned::Uuid(v) => Self::Uuid(v),
                TaggedIndexValueOwned::Timestamp(v) => Self::Timestamp(v),
                TaggedIndexValueOwned::List(v) => Self::List(v),
            })
        } else {
            let binary = BinaryIndexValueOwned::deserialize(deserializer)?;
            Ok(match binary {
                BinaryIndexValueOwned::Null => Self::Null,
                BinaryIndexValueOwned::Boolean(v) => Self::Boolean(v),
                BinaryIndexValueOwned::Integer(v) => Self::Integer(v),
                BinaryIndexValueOwned::Decimal(v) => Self::Decimal(v),
                BinaryIndexValueOwned::String(v) => Self::String(v),
                BinaryIndexValueOwned::Uuid(v) => Self::Uuid(v),
                BinaryIndexValueOwned::Timestamp(v) => Self::Timestamp(v),
                BinaryIndexValueOwned::List(v) => Self::List(v),
            })
        }
    }
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
