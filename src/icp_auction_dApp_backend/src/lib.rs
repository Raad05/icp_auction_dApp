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
    ItemNotListed,
    BidMoreForThisItem,
}

#[derive(CandidType, Deserialize)]
struct Bidder {
    bidder_id: candid::Principal,
    bid_amount: u64,
}

// structs
#[derive(CandidType, Deserialize)]
struct Item {
    name: String,
    description: String,
    is_listed: bool,
    bid_users: Vec<Bidder>,
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
fn get_listed_items_count() -> u64 {
    ITEM_MAP.with(|i| {
        let map = i.borrow();
        map.iter().filter(|(_, item)| item.is_listed).count() as u64
    })
}

#[ic_cdk::query]
fn get_item(key: u64) -> Option<Item> {
    ITEM_MAP.with(|i| i.borrow().get(&key))
}

#[ic_cdk::query]
fn get_item_sold_for_most() -> Option<Item> {
    ITEM_MAP.with(|i| {
        let map = i.borrow();
        let mut highest_bid_item: Option<Item> = None;
        let mut highest_bid_amount: u64 = 0;

        for (_, item) in map.iter() {
            if !item.is_listed {
                if let Some(value) = item.bid_users.iter().max_by_key(|b| b.bid_amount) {
                    if value.bid_amount > highest_bid_amount {
                        highest_bid_amount = value.bid_amount;
                        highest_bid_item = Some(item);
                    }
                }
            }
        }
        highest_bid_item
    })
}

#[ic_cdk::query]
fn get_item_bid_on_most() -> Option<Item> {
    ITEM_MAP.with(|i| {
        let map = i.borrow();
        let mut most_bid_item: Option<Item> = None;
        let mut most_bids_count: u64 = 0;

        for (_, item) in map.iter() {
            if !item.is_listed {
                let bids_count: u64 = item.bid_users.len() as u64;

                if bids_count > most_bids_count {
                    most_bids_count = bids_count;
                    most_bid_item = Some(item);
                }
            }
        }
        most_bid_item
    })
}

// setter functions
#[ic_cdk::update]
fn create_item(key: u64, item: CreateItem) -> Result<(), BidError> {
    let new_item: Item = Item {
        name: item.name,
        description: item.description,
        is_listed: item.is_listed,
        bid_users: vec![],
        owner: ic_cdk::caller(),
    };
    let res: Option<Item> = ITEM_MAP.with(|i| i.borrow_mut().insert(key, new_item));

    match res {
        Some(_) => Ok(()),
        None => return Err(BidError::UpdateError),
    }
}

#[ic_cdk::update]
fn edit_item(key: u64, item: CreateItem) -> Result<(), BidError> {
    ITEM_MAP.with(|i| {
        let old_item_opt: Option<Item> = i.borrow().get(&key);
        let old_item: Item;

        match old_item_opt {
            Some(value) => old_item = value,
            None => return Err(BidError::NoSuchItem),
        }

        if ic_cdk::caller() != old_item.owner {
            return Err(BidError::AccessRejected);
        }

        let edited_item: Item = Item {
            name: item.name,
            description: item.description,
            is_listed: item.is_listed,
            bid_users: old_item.bid_users,
            owner: old_item.owner,
        };

        let res: Option<Item> = i.borrow_mut().insert(key, edited_item);

        match res {
            Some(_) => Ok(()),
            None => return Err(BidError::UpdateError),
        }
    })
}

#[ic_cdk::update]
fn unlist_item(key: u64) -> Result<(), BidError> {
    ITEM_MAP.with(|i| {
        let item_opt: Option<Item> = i.borrow().get(&key);
        let mut item: Item;

        match item_opt {
            Some(value) => item = value,
            None => return Err(BidError::NoSuchItem),
        }

        if ic_cdk::caller() != item.owner {
            return Err(BidError::AccessRejected);
        }

        item.is_listed = false;

        let max_bidder: Option<&Bidder> = item.bid_users.iter().max_by_key(|b| b.bid_amount);

        match max_bidder {
            Some(value) => item.owner = value.bidder_id,
            None => return Err(BidError::UpdateError),
        }

        let res: Option<Item> = i.borrow_mut().insert(key, item);

        match res {
            Some(_) => Ok(()),
            None => return Err(BidError::UpdateError),
        }
    })
}

#[ic_cdk::update]
fn bid(key: u64, amount: u64) -> Result<(), BidError> {
    ITEM_MAP.with(|i| {
        let item_opt: Option<Item> = i.borrow().get(&key);
        let mut item: Item;

        match item_opt {
            Some(value) => item = value,
            None => return Err(BidError::NoSuchItem),
        }

        if !item.is_listed {
            return Err(BidError::ItemNotListed);
        }

        let current_highest_bid = item
            .bid_users
            .iter()
            .max_by_key(|b| b.bid_amount)
            .map_or(0, |b| b.bid_amount);

        if amount <= current_highest_bid {
            return Err(BidError::BidMoreForThisItem);
        }

        let new_bid: Bidder = Bidder {
            bidder_id: ic_cdk::caller(),
            bid_amount: amount,
        };

        item.bid_users.push(new_bid);

        let res: Option<Item> = i.borrow_mut().insert(key, item);

        match res {
            Some(_) => Ok(()),
            None => return Err(BidError::UpdateError),
        }
    })
}
