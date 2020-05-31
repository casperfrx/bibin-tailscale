extern crate gpw;
extern crate linked_hash_map;
extern crate owning_ref;
extern crate rand;

use rand::{thread_rng, Rng};

use linked_hash_map::LinkedHashMap;

use owning_ref::OwningRef;

use std::cell::RefCell;
use std::env;

use tokio::sync::{RwLock, RwLockReadGuard};

type RwLockReadGuardRef<'a, T, U = T> = OwningRef<Box<RwLockReadGuard<'a, T>>, U>;

pub enum EntryData {
    BinaryData(Vec<u8>),
    TextData(String),
}

lazy_static! {
    static ref ENTRIES: RwLock<LinkedHashMap<String, EntryData>> = RwLock::new(LinkedHashMap::new());
    static ref BUFFER_SIZE: usize = env::var("BIN_BUFFER_SIZE")
        .map(|f| f
            .parse::<usize>()
            .expect("Failed to parse value of BIN_BUFFER_SIZE"))
        .unwrap_or(2000usize);
}

/// Ensures `ENTRIES` is less than the size of `BIN_BUFFER_SIZE`. If it isn't then
/// `ENTRIES.len() - BIN_BUFFER_SIZE` elements will be popped off the front of the map.
///
/// During the purge, `ENTRIES` is locked and the current thread will block.
async fn purge_old() {
    let entries_len = ENTRIES.read().await.len();

    if entries_len > *BUFFER_SIZE {
        let to_remove = entries_len - *BUFFER_SIZE;

        let mut entries = ENTRIES.write().await;

        for _ in 0..to_remove {
            entries.pop_front();
        }
    }
}

/// Generates a 'pronounceable' random ID using gpw
pub fn generate_id(length: usize) -> String {
    thread_local!(static KEYGEN: RefCell<gpw::PasswordGenerator> = RefCell::new(gpw::PasswordGenerator::default()));

    // removed 0/o, i/1/l, u/v as they are too similar. with 4 char this gives us >700'000 unique ids
    const CHARSET: &[u8] = b"abcdefghjkmnpqrstwxyz23456789";

    (0..length)
        .map(|_| {
            let idx = thread_rng().gen_range(0, CHARSET.len());
            CHARSET[idx] as char
        })
        .collect::<String>()
}

/// Stores a paste under the given id
pub async fn store_paste(id_length: usize, content: EntryData) -> Result<String, &'static str> {
    purge_old().await;

    let mut id = generate_id(id_length);

    let mut guard = ENTRIES.write().await;
    let mut remaining_attempts = 5;
    while guard.contains_key(&id) {
        println!("WARNING: id collision");
        id = generate_id(id_length);

        remaining_attempts -= 1;
        if remaining_attempts == 0 {
            return Err("Could not find a suitable ID.");
        }
    }
    guard.insert(id.clone(), content);
    Ok(id)
}

/// Get a paste by id.
///
/// Returns `None` if the paste doesn't exist.
pub async fn get_paste(
    id: &str,
) -> Option<RwLockReadGuardRef<'_, LinkedHashMap<String, EntryData>, EntryData>> {
    // need to box the guard until owning_ref understands Pin is a stable address
    let or = RwLockReadGuardRef::new(Box::new(ENTRIES.read().await));

    if or.contains_key(id) {
        Some(or.map(|x| x.get(id).unwrap()))
    } else {
        None
    }
}
