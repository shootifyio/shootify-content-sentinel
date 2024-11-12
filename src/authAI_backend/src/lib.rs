use std::{borrow::Cow, cell::RefCell};

use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use ic_cdk::caller;
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, Storable,
};

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(CandidType, Deserialize)]
struct Address {
    street: String,
    city: String,
    state: String,
    zip: u64,
    created_by: Principal,
}

impl Storable for Address {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    const BOUND: Bound = Bound::Unbounded;
}

thread_local! {
    // The memory manager is used for simulating multiple memories. Given a `MemoryId` it can
    // return a memory that can be used by stable structures.
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    // Initialize a `StableBTreeMap` with `MemoryId(0)`.
    static STABLE_ADDRESSES: RefCell<StableBTreeMap<String, Address, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
        )
    );
}

#[derive(CandidType, Deserialize)]
struct AddressInput {
    street: String,
    city: String,
    state: String,
    zip: u64,
}

#[ic_cdk::update]
fn set_address(name: String, input: AddressInput) {
    let caller = caller();
    STABLE_ADDRESSES.with_borrow_mut(|s| {
        s.insert(
            name,
            Address {
                street: input.street,
                city: input.city,
                state: input.state,
                zip: input.zip,
                created_by: caller,
            },
        )
    });
}

#[ic_cdk::query]
fn get_address(name: String) -> Option<Address> {
    STABLE_ADDRESSES.with_borrow(|s| s.get(&name))
}
