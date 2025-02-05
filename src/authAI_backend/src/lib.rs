use std::{borrow::Cow, cell::RefCell};

use candid::{CandidType, Decode, Encode, Principal};
use ic_cdk::caller;
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, Storable,
};

type Memory = VirtualMemory<DefaultMemoryImpl>;
use serde::{Deserialize, Serialize};


#[derive(CandidType, Serialize, Deserialize)]
struct StoredImage {
    content: Vec<u8>,
    uploaded_by: Principal,
}

impl Storable for StoredImage {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    const BOUND: Bound = Bound::Unbounded;
}

thread_local! {
    // Memory manager for handling stable memory
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    // Stable BTreeMap to store images, using a string key (e.g., image name)
    static STABLE_IMAGES: RefCell<StableBTreeMap<String, StoredImage, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
        )
    );
}

/// Store an image in the stable memory
#[ic_cdk::update]
fn store_image(name: String, content: Vec<u8>) -> Result<(), String> {
    if name.is_empty() {
        return Err("Image name cannot be empty.".to_string());
    }
    if content.is_empty() {
        return Err("Image content cannot be empty.".to_string());
    }

    let uploader = caller();
    STABLE_IMAGES.with_borrow_mut(|images| {
        images.insert(
            name.clone(),
            StoredImage {
                content,
                uploaded_by: uploader,
            },
        );
    });
    ic_cdk::println!("Image '{}' stored successfully by {}", name, uploader);
    Ok(())
}

/// Retrieve an image from the stable memory by its name
#[ic_cdk::query]
fn get_image(name: String) -> Option<StoredImage> {
    STABLE_IMAGES.with_borrow(|images| images.get(&name))
}


/// List all stored image names
#[ic_cdk::query]
fn list_images() -> Vec<String> {
    STABLE_IMAGES.with_borrow(|images| {
        images.iter().map(|(key, _)| key.clone()).collect()
    })
}

/// Delete an image by its name
#[ic_cdk::update]
fn delete_image(name: String) -> Result<(), String> {
    let exists = STABLE_IMAGES.with_borrow_mut(|images| images.remove(&name).is_some());
    if exists {
        ic_cdk::println!("Image '{}' deleted successfully", name);
        Ok(())
    } else {
        Err(format!("Image '{}' not found.", name))
    }
}