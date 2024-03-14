use std::{fs::File, sync::{Arc, Mutex, RwLock, Weak}, time::Duration};

use crate::{common::{self, meta::Meta}, tx::Tx};
use  crate::errors::Result;
struct freelist;
struct batch;

struct Stats;


// FreelistType enum (replace with actual variants)
enum FreelistType {
    Array,
    HashMap,
}



pub(crate) struct RawDB {

    stats: Arc<Mutex<Stats>>, // Thread-safe access to statistics

    // Flags with explicit defaults
    strict_mode: bool,
    no_sync: bool,
    no_freelist_sync: bool,
    freelist_type: FreelistType,
    no_grow_sync: bool,
    pre_load_freelist: bool,
    mmap_flags: i32,

    // Configuration options
    max_batch_size: isize,
    max_batch_delay: Duration,
    alloc_size: usize,
    mlock: bool,

    // logger: Option<Logger>, // Optional logger

    path: String,
    file: Option<Arc<Mutex<File>>>, // Thread-safe file handle
    dataref: Option<Vec<u8>>, // Optional mmap'ed data (read-only)
    data: Option<Box<[u8]>>, // Optional data pointer (writeable)
    datasz: usize,

    meta0: Option<Arc<Mutex<Meta>>>, // Thread-safe meta page 0
    meta1: Option<Arc<Mutex<Meta>>>, // Thread-safe meta page 1

    page_size: usize,

    opened: bool,
    rwtx: Option<Arc<Mutex<Tx>>>, // Read-write transaction (writer)
    txs: Vec<Arc<Mutex<Tx>>>, // Read-only transactions

    freelist: Option<Arc<Mutex<freelist>>>, // Thread-safe freelist access
    freelist_load: Mutex<bool>, // Flag to track freelist loading

    page_pool: Mutex<Vec<Box<[u8]>>>, // Pool of allocated pages

    batch_mu: Mutex<Option<batch>>, // Mutex for batch operations
    rwlock: Mutex<()>, // Mutex for single writer access

    metalock: Mutex<()>, // Mutex for meta page access
    mmaplock: RwLock<()>, // RWLock for mmap access during remapping

    statlock: RwLock<()>, // RWLock for stats access

    ops: Ops, // Operations struct for file access

    read_only: bool, // Read-only mode flag

}

struct Ops {
    write_at: fn(&[u8], i64) -> Result<usize>,
}


#[derive(Clone)]
pub struct DB(pub(crate) Arc<RawDB>);

#[derive(Clone, Debug)]
pub(crate) struct WeakDB(Weak<RawDB>);

impl WeakDB {
    pub(crate) fn new() -> WeakDB {
        WeakDB(Weak::new())
    }

    pub(crate) fn upgrade(&self) -> Option<DB> {
        self.0.upgrade().map(DB)
    }

    pub(crate) fn from(db: &DB) -> WeakDB {
        WeakDB(Arc::downgrade(&db.0))
    }
}
