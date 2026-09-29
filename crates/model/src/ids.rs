use std::{cmp::Ordering, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::ModelError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct AvdId(String);

impl AvdId {
    pub fn new(value: impl Into<String>) -> Result<Self, ModelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ModelError::EmptyIdentifier);
        }
        if value == "." || value == ".." || value.contains('/') || value.contains('\\') {
            return Err(ModelError::InvalidIdentifier(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AvdId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Serial(String);

impl Serial {
    pub fn new(value: impl Into<String>) -> Result<Self, ModelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ModelError::EmptyIdentifier);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Serial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn new(value: impl Into<String>) -> Result<Self, ModelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ModelError::EmptyIdentifier);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

macro_rules! deserialize_validated_identifier {
    ($identifier:ty) => {
        impl<'de> Deserialize<'de> for $identifier {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

deserialize_validated_identifier!(AvdId);
deserialize_validated_identifier!(Serial);
deserialize_validated_identifier!(ProfileId);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageId {
    pub kind: PackageKind,
    pub api: Option<String>,
    pub tag: Option<String>,
    pub abi: Option<String>,
    pub qualifier: Option<String>,
}

impl PackageId {
    pub fn segments(&self) -> Vec<String> {
        let mut segments = vec![self.kind.directory().to_owned()];
        if let Some(api) = &self.api {
            if api.starts_with("android-") || self.kind != PackageKind::SystemImage {
                segments.push(api.clone());
            } else {
                segments.push(format!("android-{api}"));
            }
        }
        if let Some(tag) = &self.tag {
            segments.push(tag.clone());
        }
        if let Some(abi) = &self.abi {
            segments.push(abi.clone());
        }
        if let Some(qualifier) = &self.qualifier {
            segments.push(qualifier.clone());
        }
        segments
    }

    pub fn render_semicolon(&self) -> String {
        self.segments().join(";")
    }

    pub fn render_slash(&self) -> String {
        self.segments().join("/")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    SystemImage,
    Platform,
    BuildTools,
    Emulator,
    PlatformTools,
    CommandLineTools,
    Other,
}

impl PackageKind {
    pub fn directory(self) -> &'static str {
        match self {
            Self::SystemImage => "system-images",
            Self::Platform => "platforms",
            Self::BuildTools => "build-tools",
            Self::Emulator => "emulator",
            Self::PlatformTools => "platform-tools",
            Self::CommandLineTools => "cmdline-tools",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    pub components: Vec<u64>,
    pub suffix: Option<String>,
}

impl FromStr for Revision {
    type Err = ModelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (numbers, suffix) = value
            .split_once('-')
            .map_or((value, None), |(a, b)| (a, Some(b)));
        let mut components = numbers
            .split('.')
            .map(|component| {
                component
                    .parse()
                    .map_err(|_| ModelError::InvalidRevision(value.into()))
            })
            .collect::<Result<Vec<u64>, _>>()?;
        while components.last() == Some(&0) && components.len() > 1 {
            components.pop();
        }
        if components.is_empty() {
            return Err(ModelError::InvalidRevision(value.into()));
        }
        Ok(Self {
            components,
            suffix: suffix.filter(|value| !value.is_empty()).map(str::to_owned),
        })
    }
}

impl PartialEq for Revision {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Revision {}

impl Ord for Revision {
    fn cmp(&self, other: &Self) -> Ordering {
        let length = self.components.len().max(other.components.len());
        for index in 0..length {
            let left = self.components.get(index).copied().unwrap_or_default();
            let right = other.components.get(index).copied().unwrap_or_default();
            match left.cmp(&right) {
                Ordering::Equal => continue,
                ordering => return ordering,
            }
        }
        match (self.suffix.as_deref(), other.suffix.as_deref()) {
            (None | Some(""), None | Some("")) => Ordering::Equal,
            (left, right) => left.cmp(&right),
        }
    }
}

impl PartialOrd for Revision {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_compare_without_trailing_zero_significance() {
        let left: Revision = "7".parse().unwrap();
        let right: Revision = "7.0.0".parse().unwrap();
        assert_eq!(left, right);
        assert_eq!(left.cmp(&right), Ordering::Equal);
    }

    #[test]
    fn package_id_renders_system_image_paths() {
        let id = PackageId {
            kind: PackageKind::SystemImage,
            api: Some("36.1".into()),
            tag: Some("google_apis".into()),
            abi: Some("arm64-v8a".into()),
            qualifier: None,
        };
        assert_eq!(
            id.render_semicolon(),
            "system-images;android-36.1;google_apis;arm64-v8a"
        );
        assert_eq!(
            id.render_slash(),
            "system-images/android-36.1/google_apis/arm64-v8a"
        );
    }

    #[test]
    fn identifier_deserialization_preserves_constructor_invariants() {
        assert!(serde_json::from_str::<AvdId>(r#""../phone""#).is_err());
        assert!(serde_json::from_str::<Serial>(r#""""#).is_err());
        assert!(serde_json::from_str::<ProfileId>(r#""""#).is_err());

        let id = serde_json::from_str::<AvdId>(r#""phone""#).unwrap();
        assert_eq!(id.as_str(), "phone");
    }
}
