use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};
use std::{fmt, marker::PhantomData};

/// Explicit mutation semantics for nullable owner fields.
///
/// A missing field deserializes as [`Patch::Keep`], an explicit JSON/GraphQL
/// `null` maps to [`Patch::Clear`], and a concrete value maps to
/// [`Patch::Set`]. This prevents update commands from conflating "not
/// supplied" with "clear the current value".
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Patch<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}

impl<T> Patch<T> {
    pub const fn is_keep(&self) -> bool {
        matches!(self, Self::Keep)
    }

    pub const fn is_changed(&self) -> bool {
        !self.is_keep()
    }

    pub fn as_ref(&self) -> Patch<&T> {
        match self {
            Self::Keep => Patch::Keep,
            Self::Set(value) => Patch::Set(value),
            Self::Clear => Patch::Clear,
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Patch<U> {
        match self {
            Self::Keep => Patch::Keep,
            Self::Set(value) => Patch::Set(f(value)),
            Self::Clear => Patch::Clear,
        }
    }
}

impl<T: Serialize> Serialize for Patch<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Keep | Self::Clear => serializer.serialize_none(),
            Self::Set(value) => value.serialize(serializer),
        }
    }
}

struct PatchVisitor<T>(PhantomData<T>);

impl<'de, T> Visitor<'de> for PatchVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = Patch<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a value or null")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Patch::Clear)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Patch::Clear)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Patch::Set)
    }
}

impl<'de, T> Deserialize<'de> for Patch<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_option(PatchVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::Patch;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize)]
    struct Example {
        #[serde(default, skip_serializing_if = "Patch::is_keep")]
        value: Patch<String>,
    }

    #[test]
    fn serde_distinguishes_keep_clear_and_set() {
        let keep: Example = serde_json::from_str("{}").expect("missing field");
        assert_eq!(keep.value, Patch::Keep);

        let clear: Example = serde_json::from_str(r#"{"value":null}"#).expect("null field");
        assert_eq!(clear.value, Patch::Clear);

        let set: Example = serde_json::from_str(r#"{"value":"next"}"#).expect("value field");
        assert_eq!(set.value, Patch::Set("next".to_string()));

        assert_eq!(
            serde_json::to_value(keep).expect("serialize keep"),
            serde_json::json!({})
        );
        assert_eq!(
            serde_json::to_value(clear).expect("serialize clear"),
            serde_json::json!({"value": null})
        );
    }
}
