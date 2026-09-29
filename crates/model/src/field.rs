use serde::{Deserialize, Serialize};

/// Three-state field: present, not provided by the current tool, or not applicable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Field<T> {
    /// A reliable value is available.
    Present {
        /// Field value.
        value: T,
    },
    /// The current implementation or tool did not provide a value.
    Unavailable,
    /// The field does not apply to this object or state.
    Inapplicable,
}

impl<T> Field<T> {
    /// Creates a present field.
    pub fn present(value: T) -> Self {
        Self::Present { value }
    }

    /// Borrows the present value, if any.
    pub fn as_value(&self) -> Option<&T> {
        match self {
            Self::Present { value } => Some(value),
            Self::Unavailable | Self::Inapplicable => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_a_state_tag() {
        let present = serde_json::to_value(Field::present("pixel")).unwrap();
        assert_eq!(present["state"], "present");
        assert_eq!(present["value"], "pixel");

        let unavailable = serde_json::to_value(Field::<String>::Unavailable).unwrap();
        assert_eq!(unavailable, serde_json::json!({ "state": "unavailable" }));
    }
}
