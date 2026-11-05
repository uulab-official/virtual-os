//! RFC 3339 timestamps used throughout the persisted model.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// A UTC timestamp serialized as an RFC 3339 string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub OffsetDateTime);

impl Timestamp {
    pub fn now() -> Self {
        Self(OffsetDateTime::now_utc())
    }

    pub fn from_unix(seconds: i64) -> Self {
        Self(OffsetDateTime::from_unix_timestamp(seconds).unwrap_or(OffsetDateTime::UNIX_EPOCH))
    }

    pub fn unix_seconds(self) -> i64 {
        self.0.unix_timestamp()
    }

    pub fn to_rfc3339(self) -> String {
        self.0
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self(OffsetDateTime::UNIX_EPOCH)
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        OffsetDateTime::parse(&value, &Rfc3339)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let stamp = Timestamp::from_unix(1_760_000_000);
        let json = serde_json::to_string(&stamp).unwrap();
        assert_eq!(json, "\"2025-10-09T08:53:20Z\"");
        let parsed: Timestamp = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.unix_seconds(), 1_760_000_000);
    }
}
