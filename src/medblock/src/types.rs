use candid::{CandidType, Deserialize};
use ic_stable_memory::{
    collections::SHashMap,
    derive::{AsFixedSizeBytes, StableType},
    SBox,
};
use uuid::Uuid;

use crate::deref;

#[derive(
    CandidType, StableType, AsFixedSizeBytes, Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Debug,
)]
pub struct Timestamp(u64);

/// emr metadata key must not exceed 100 ascii characters
#[derive(
    CandidType, StableType, AsFixedSizeBytes, Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Debug,
)]
pub struct EmrMetadataKey([u8; 100]);
deref!(EmrMetadataKey: [u8; 100]);

/// wrapper for [uuid::Uuid] because candid is not implemented for [uuid::Uuid]
#[derive(
    CandidType, StableType, AsFixedSizeBytes, Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Debug,
)]
pub struct ID([u8; 16]);

impl ID {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for ID {
    fn default() -> Self {
        uuid::Uuid::new_v4().into_bytes().into()
    }
}

impl From<Uuid> for ID {
    fn from(value: Uuid) -> Self {
        Self(value.into_bytes())
    }
}

deref!(ID: Uuid |_self| => &Uuid::from_bytes_ref(&_self.0));
