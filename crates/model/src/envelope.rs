use serde::{Deserialize, Serialize};

/// Current version of the serialized outlet contract.
pub const SCHEMA_VERSION: &str = "0.1.0";

/// Versioned wrapper used for complete JSON responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Version of the serialized data contract.
    pub schema_version: String,
    /// Response payload.
    pub data: T,
}

impl<T> Envelope<T> {
    /// Wraps a payload using the current schema version.
    pub fn new(data: T) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_owned(),
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_includes_schema_version() {
        let encoded = serde_json::to_value(Envelope::new("ok")).unwrap();
        assert_eq!(encoded["schema_version"], SCHEMA_VERSION);
        assert_eq!(encoded["data"], "ok");
    }
}
