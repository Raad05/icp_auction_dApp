use candid::{CandidType, Decode, Deserialize, Encode};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;

const MAX_VALUE_SIZE: u32 = 100;

// enums
#[derive(CandidType, Deserialize)]
enum BidError {
    AlreadyBid,
    NoSuchItem,
    AccessRejected,
    UpdateError,
}

// structs
#[derive(CandidType, Deserialize)]
struct Item {
    name: String,
    description: String,
    is_listed: bool,
    bid_users: Vec<candid::Principal>,
    owner: candid::Principal,
}

#[derive(CandidType, Deserialize)]
struct CreateItem {
    name: String,
    description: String,
    is_listed: bool,
}

// implementations
impl Storable for Item {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Item {
    const MAX_SIZE: u32 = MAX_VALUE_SIZE;
    const IS_FIXED_SIZE: bool = false;
}

// thread_local
thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static ITEM_MAP:RefCell<StableBTreeMap<u64, Item, Memory>> = RefCell::new(StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0)))));
}

// getter functions
#[ic_cdk::query]
fn get_all_items() -> Vec<Item> {
    ITEM_MAP.with(|i| {
        let map = i.borrow();
        let mut items: Vec<Item> = Vec::new();

        for (_, item) in map.iter() {
            items.push(item);
        }
        items
    })
}

#[ic_cdk::query]
fn get_item(key: u64) -> Option<Item> {
    ITEM_MAP.with(|i| i.borrow().get(&key))
}
