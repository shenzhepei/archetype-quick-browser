use std::{error::Error, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize};
use url::Url;
use uuid::{Uuid, Version};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct PageId(String);

impl PageId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for PageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for PageId {
    type Err = ParsePageIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::parse_str(value).map_err(|_| ParsePageIdError)?;
        if uuid.get_version() != Some(Version::SortRand) {
            return Err(ParsePageIdError);
        }
        Ok(Self(uuid.to_string()))
    }
}

impl<'de> Deserialize<'de> for PageId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParsePageIdError;

impl fmt::Display for ParsePageIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid UUID v7 page ID")
    }
}

impl Error for ParsePageIdError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ArchetypeUrl(String);

impl ArchetypeUrl {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArchetypeUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ArchetypeUrl {
    type Err = ParseArchetypeUrlError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(value).map_err(|_| ParseArchetypeUrlError)?;
        Ok(Self(url.to_string()))
    }
}

impl<'de> Deserialize<'de> for ArchetypeUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseArchetypeUrlError;

impl fmt::Display for ParseArchetypeUrlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid absolute URL")
    }
}

impl Error for ParseArchetypeUrlError {}

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct NavigationId(u64);

impl NavigationId {
    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn saturating_next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum LoadStage {
    Loading,
    Parsed,
    LaidOut,
    Ready,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_id_uses_a_valid_uuid_v7_string() {
        let page_id = PageId::new();
        let parsed = Uuid::parse_str(page_id.as_str()).unwrap();
        assert_eq!(parsed.get_version(), Some(Version::SortRand));
        assert_eq!(page_id.to_string(), page_id.as_str());
    }

    #[test]
    fn page_id_json_is_a_validated_string() {
        let page_id = PageId::new();
        let json = serde_json::to_string(&page_id).unwrap();
        assert_eq!(json, format!("\"{page_id}\""));
        assert_eq!(serde_json::from_str::<PageId>(&json).unwrap(), page_id);
        assert!(serde_json::from_str::<PageId>("\"not-a-uuid\"").is_err());
    }

    #[test]
    fn page_id_rejects_non_v7_uuids() {
        assert!(
            "550e8400-e29b-41d4-a716-446655440000"
                .parse::<PageId>()
                .is_err()
        );
    }

    #[test]
    fn archetype_url_json_is_a_validated_string() {
        let url = "https://example.com/a%20path?query=value"
            .parse::<ArchetypeUrl>()
            .unwrap();
        let json = serde_json::to_string(&url).unwrap();
        assert_eq!(json, format!("\"{url}\""));
        assert_eq!(serde_json::from_str::<ArchetypeUrl>(&json).unwrap(), url);
        assert!(serde_json::from_str::<ArchetypeUrl>("\"relative/path\"").is_err());
    }

    #[test]
    fn load_stage_has_a_stable_json_shape() {
        let json = serde_json::to_string(&LoadStage::Cancelled).unwrap();
        assert_eq!(json, "\"cancelled\"");
        assert_eq!(
            serde_json::from_str::<LoadStage>(&json).unwrap(),
            LoadStage::Cancelled
        );
        assert!(serde_json::from_str::<LoadStage>("\"unknown\"").is_err());
    }

    #[test]
    fn navigation_id_has_a_stable_numeric_shape_and_saturates() {
        let zero = NavigationId::zero();
        assert_eq!(zero.get(), 0);
        assert_eq!(serde_json::to_string(&zero).unwrap(), "0");
        assert_eq!(
            serde_json::from_str::<NavigationId>("42").unwrap().get(),
            42
        );

        let maximum = NavigationId(u64::MAX);
        assert_eq!(maximum.saturating_next(), maximum);
    }
}
