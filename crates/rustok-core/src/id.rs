use ulid::Ulid;
use uuid::Uuid;

use crate::error::{Error, Result};

pub fn generate_id() -> Uuid {
    Uuid::from_bytes(Ulid::r#gen().to_bytes())
}

pub fn parse_id(value: &str) -> Result<Uuid> {
    value
        .parse::<Ulid>()
        .map(|ulid| Uuid::from_bytes(ulid.to_bytes()))
        .or_else(|_| value.parse::<Uuid>())
        .map_err(|_| Error::InvalidIdFormat(value.to_string()))
}
