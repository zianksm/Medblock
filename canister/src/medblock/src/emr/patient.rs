use candid::{CandidType, Principal};
use ic_stable_memory::{
    collections::{SBTreeMap, SBTreeSet},
    derive::{AsFixedSizeBytes, StableType},
    primitive::s_ref::SRef,
};

use crate::{deref, types::Id};

type EmrId = Id;
const KEY_LEN: usize = 32;

/// SHA3-256 hash of NIK, used as key for [BindingMap].
/// we can't check for hash validity, so we assume it's valid by checking it's length.
#[derive(StableType, AsFixedSizeBytes, Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Debug)]
pub struct InternalBindingKey([u8; KEY_LEN]);

impl InternalBindingKey {
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("key must be ascii")
    }
}

deref!(InternalBindingKey: [u8; KEY_LEN]);

mod deserialize {
    use super::*;

    impl<'de> serde::Deserialize<'de> for InternalBindingKey {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?.into_bytes();

            if s.len() != KEY_LEN {
                return Err(serde::de::Error::custom("invalid nik hash length"));
            }

            // TODO: unnecessary copy
            let mut key = [0u8; KEY_LEN];
            key[..s.len()].copy_from_slice(&s);

            Ok(Self(key))
        }
    }

    impl CandidType for InternalBindingKey {
        fn _ty() -> candid::types::Type {
            candid::types::Type::Text
        }

        fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
        where
            S: candid::types::Serializer,
        {
            serializer.serialize_text(self.as_str())
        }
    }
}

pub type Owner = Principal;
pub type NIK = InternalBindingKey;
/// Principal to NIK Map. meant to enforce 1:1 relationship between principal and NIK.
/// used to claim emrs ownership. This level of inderction is needed because principal that map to a particular BindingKey effectively owns
/// all the emrs that it's BindingKey map to.
#[derive(Default)]
pub struct OwnerMap(SBTreeMap<Owner, NIK>);

impl OwnerMap {
    pub fn get_nik(&self, owner: &Owner) -> Option<SRef<'_, NIK>> {
        self.0.get(owner)
    }

    pub fn new() -> Self {
        Self::default()
    }
}

deref!(mut OwnerMap: SBTreeMap<Owner, NIK>);

pub type EmrIdCollection = SBTreeSet<EmrId>;
/// track emr issued for a particular user by storing it's emr id in this map. also used as blind index for emr search.
/// we use hashed (keccak256) NIK as key and emr id as value.
///
/// we don't use the principal directly because we want users to be able to change it's internet identity
/// and still be able to own and access their emr.
///
/// NIK MUST be hashed offchain before being used as key.
#[derive(Default)]
pub struct EmrBindingMap(SBTreeMap<NIK, EmrIdCollection>);

deref!(mut EmrBindingMap: SBTreeMap<NIK, EmrIdCollection>);

impl EmrBindingMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_owner_of(&self, nik: &NIK, emr_id: &EmrId) -> bool {
        self.0
            .get(nik)
            .map(|emr_ids| emr_ids.contains(emr_id))
            .unwrap_or(false)
    }
}