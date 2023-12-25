pub mod patient;
pub mod providers;

use candid::{ CandidType, Principal };
use ic_stable_memory::{
    collections::SHashMap,
    derive::{ AsFixedSizeBytes, StableType },
    primitive::s_ref::SRef,
    AsFixedSizeBytes,
    SBox,
    StableType,
};
use serde::Deserialize;

/// marker for types that can be serialized as response, it basically have 2 requirements
/// and that is candid type and cloneable. this works because while stable memory type may implement
/// candid, it cannot implement clone 'safely' as cloning a stable memory data involves
/// allocating stable memory in which it may fail due to memory exhaustion.
pub trait ResonpseMarker: CandidType + Clone + FromStableRef {}

/// this basically enforce that response maker type is only be able to be created from stable memory reference, effectively mirroring the stable memory data to heap
pub trait FromStableRef {
    type From: StableType;

    fn from_stable_ref(sref: &Self::From) -> Self;
}

pub trait ToResponse<T: ResonpseMarker> {
    fn to_response(&self) -> T;
}

use crate::{ deref, measure_alloc, types::{ AsciiRecordsKey, Id, Timestamp } };

use self::{ patient::{ EmrBindingMap, OwnerMap, NIK, InternalBindingKey } };

#[derive(Default)]
pub struct EmrRegistry {
    owners: OwnerMap,
    owner_emrs: EmrBindingMap,
    core_emrs: EmrCollection,
}

impl EmrRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_emr(
        &mut self,
        emr: Emr,
        user_id: InternalBindingKey
    ) -> Result<(), OutOfMemory> {
        let emr_id = emr.id().clone();

        self.core_emrs.new_emr(emr)?;
        self.owner_emrs.issue_for(&user_id, emr_id);

        Ok(())
    }

    /// register new patient to the system, returns [OutOfMemory] if stable memory is exhausted
    pub fn register_patient(
        &mut self,
        owner: Principal,
        hashed_nik: NIK
    ) -> Result<(), OutOfMemory> {
        self.owners.bind(owner, hashed_nik)
    }

    /// rebind patient to a new hashed_nik, returns [OutOfMemory] if stable memory is exhausted
    pub fn rebind_patient(&mut self, owner: Principal, hashed_nik: NIK) -> Result<(), OutOfMemory> {
        self.owners.bind(owner, hashed_nik)
    }

    /// revoke patient access, if this method is called then the patient will no longer be able to access their emr. it will remove the [NIK]
    /// from the owner map so attempting to access NIK owner will fail.
    pub fn revoke_patient_access(&mut self, owner: &Principal) {
        self.owners.revoke(owner)
    }

    pub fn is_owner_of_emr(&self, owner: &Principal, emr_id: &Id) -> bool {
        let Some(nik) = self.owners.get_nik(owner) else {
            return false;
        };

        self.owner_emrs.is_owner_of(&nik, emr_id)
    }

    pub fn is_valid_patient(&self, owner: &patient::Owner) -> bool {
        self.owners.is_valid_owner(owner)
    }

    pub fn get_emr(&self, emr_id: &Id) -> Option<SRef<'_, Emr>> {
        self.core_emrs.get_emr(emr_id)
    }
}

type EmrId = Id;
#[derive(Default)]
pub struct EmrCollection(ic_stable_memory::collections::SBTreeMap<EmrId, Emr>);

impl EmrCollection {
    pub fn get_emr(&self, emr_id: &EmrId) -> Option<SRef<'_, Emr>> {
        self.0.get(emr_id)
    }

    pub fn new_emr(&mut self, emr: Emr) -> Result<EmrId, OutOfMemory> {
        let emr_id = emr.id().clone();

        self.0.insert(emr_id.clone(), emr)?;

        Ok(emr_id)
    }
}
deref!(mut EmrCollection: ic_stable_memory::collections::SBTreeMap<EmrId,Emr>);
measure_alloc!("emr_collection_with_10_thousands_emr_10_records": {
    let mut emr_collection = EmrCollection::default();

    for _i in 0..10_000 {
        let mut emr = V001::default();

        for i in 0..10 {
            emr.records.insert(
                AsciiRecordsKey::new(format!("test{}", i)).unwrap(),
                EmrRecordsValue::new(format!("test{}", i)).unwrap(),
            );
        }

        emr_collection.insert(
            Id::new(),
            Emr::V001(V001::default()),
        );
    }



    emr_collection
});
/// version aware emr
#[derive(StableType, AsFixedSizeBytes, Debug)]
pub enum Emr {
    V001(V001),
}

impl TryFrom<EmrDisplay> for Emr {
    type Error = String;

    fn try_from(value: EmrDisplay) -> Result<Self, Self::Error> {
        match value {
            EmrDisplay::V001(v) => Ok(Self::V001(V001::try_from(v)?)),
        }
    }
}

impl std::cmp::Eq for Emr {}

impl std::cmp::PartialEq for Emr {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Emr {
    pub fn id(&self) -> &Id {
        match self {
            Self::V001(v) => &v.emr_id,
        }
    }
}

impl std::cmp::Ord for Emr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

impl std::cmp::PartialOrd for Emr {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id().partial_cmp(other.id())
    }
}

#[derive(Clone, CandidType)]
pub enum EmrDisplay {
    V001(DisplayV001),
}

impl ResonpseMarker for EmrDisplay {}

impl FromStableRef for EmrDisplay {
    type From = Emr;

    fn from_stable_ref(sref: &Emr) -> Self {
        match sref {
            Emr::V001(v) => Self::V001(DisplayV001::from_stable_ref(v)),
        }
    }
}

/// Error when allocating something to stable memory due to stable memory exhaustion
#[derive(Debug)]
pub struct OutOfMemory;

impl<T> From<T> for OutOfMemory where T: StableType {
    fn from(_: T) -> Self {
        Self
    }
}

impl std::error::Error for OutOfMemory {}

impl std::fmt::Display for OutOfMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stable memory exhausted")
    }
}

/// wrapper types for emr records, essentially just a [SBox] around [String].
/// required because we need to implement function to serialize this to [serde_json::Value] for [Records] type
#[derive(StableType, Debug, AsFixedSizeBytes)]
pub struct EmrRecordsValue(SBox<String>);
deref!(EmrRecordsValue: SBox<String>);

impl EmrRecordsValue {
    pub fn value_from_ref(&self) -> serde_json::Value {
        self.0.as_str().into()
    }

    /// create new [EmrRecordsValue] from [String], returns [OutOfMemory] if stable memory is exhausted
    pub fn new(value: impl Into<String>) -> Result<EmrRecordsValue, OutOfMemory> {
        let value = value.into();
        let value = SBox::new(value)?;

        Ok(Self(value))
    }
}

#[derive(StableType, Debug, AsFixedSizeBytes, Default)]
pub struct Records(SHashMap<AsciiRecordsKey, EmrRecordsValue>);
deref!(mut Records: SHashMap<AsciiRecordsKey, EmrRecordsValue>);

measure_alloc!("records": {
       let mut records = Records::default();

       records.insert(
           AsciiRecordsKey::new("test").unwrap(),
           EmrRecordsValue::new("test").unwrap(),
       );

       records
});

impl Clone for Records {
    fn clone(&self) -> Self {
        todo!();

        // TODO : fix this, we're using stable memory as our main memory, but there is some case
        // such as cloning a emr copy that would result in stable memory allocation while we want to use the heap for that  as we didn't store any data after
        // the response has been serialized and sent. it's like using hard disk as a ram, but you want the volatility of ram.
    }
}

impl Records {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn to_value(&self) -> serde_json::Value {
        self.0
            .iter()
            .map(|(k, v)| (k.to_string(), v.value_from_ref()))
            .collect()
    }
}

impl TryFrom<RecrodsDisplay> for Records {
    type Error = String;

    fn try_from(value: RecrodsDisplay) -> Result<Self, Self::Error> {
        let value = value.0;

        let mut records = Records::default();

        for (k, v) in value.as_object().unwrap() {
            records
                .insert(
                    AsciiRecordsKey::new(k).map_err(|e| e.to_string())?,
                    EmrRecordsValue::new(v.as_str().unwrap()).map_err(|e| e.to_string())?
                )
                .map_err(OutOfMemory::from)
                .map_err(|e| e.to_string())?;
        }

        Ok(records)
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct RecrodsDisplay(serde_json::Value);

impl ToString for RecrodsDisplay {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl ResonpseMarker for RecrodsDisplay {}

impl FromStableRef for RecrodsDisplay {
    type From = Records;

    fn from_stable_ref(sref: &Records) -> Self {
        Self(sref.to_value())
    }
}

impl CandidType for RecrodsDisplay {
    fn _ty() -> candid::types::Type {
        candid::types::Type::Text
    }

    // TODO:  this copies ALOT of data
    // because we iterate and serializing the data to serde json Value type while copying
    // and then after that we copy again to serialize the Value type to String so that it can be properly serialized as candid type
    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
        where S: candid::types::Serializer
    {
        String::idl_serialize(&self.to_string(), serializer)
    }
}

#[derive(AsFixedSizeBytes, StableType, Debug, Clone)]
pub struct V001 {
    emr_id: Id,
    created_at: Timestamp,
    updated_at: Timestamp,
    records: Records,
}

impl V001 {
    pub fn new(id: Id, records: Records) -> Self {
        Self {
            emr_id: id,
            created_at: Timestamp::new(),
            updated_at: Timestamp::new(),
            records,
        }
    }
}

impl TryFrom<DisplayV001> for V001 {
    type Error = String;

    fn try_from(value: DisplayV001) -> Result<Self, Self::Error> {
        let records = Records::try_from(value.records)?;

        Ok(Self {
            emr_id: value.emr_id,
            created_at: value.created_at,
            updated_at: value.updated_at,
            records,
        })
    }
}

measure_alloc!("emr_with_10_records":{
    let mut emr = V001::default();

    for i in 0..10 {
        emr.records.insert(
            AsciiRecordsKey::new(format!("test{}", i)).unwrap(),
            EmrRecordsValue::new(format!("test{}", i)).unwrap(),
        );
    }

    emr
});

impl FromStableRef for DisplayV001 {
    type From = V001;

    fn from_stable_ref(sref: &V001) -> Self {
        Self {
            emr_id: sref.emr_id.clone(),
            created_at: sref.created_at,
            updated_at: sref.updated_at,
            records: RecrodsDisplay::from_stable_ref(&sref.records),
        }
    }
}
#[derive(Debug, CandidType, Clone, Deserialize)]
pub struct DisplayV001 {
    emr_id: Id,
    created_at: Timestamp,
    updated_at: Timestamp,
    records: RecrodsDisplay,
}
