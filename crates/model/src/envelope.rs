use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub schema_version: String,
    pub data: T,
}

impl<T> Envelope<T> {
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
