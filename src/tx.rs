use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::bucket::Bucket;
use crate::common::meta::Meta;
use crate::common::page::{OwnedPage, PgId};
use crate::db::WeakDB;

// Tx represents a read-only or read/write transaction on the database.
// Read-only transactions can be used for retrieving values for keys and creating cursors.
// Read/write transactions can create and remove buckets and create and remove keys.
//
// IMPORTANT: You must commit or rollback transactions when you are done with
// them. Pages can not be reclaimed by the writer until no more transactions
// are using them. A long running read transaction can cause the database to
// quickly grow.

pub struct RawTx {
    writable: AtomicBool,
    managed: AtomicBool,
    db: RwLock<WeakDB>,
    /// transaction meta
    meta: RwLock<Meta>,
    /// root bucket
    root: RwLock<Bucket>,
    /// cache page
    pages: RwLock<HashMap<PgId, OwnedPage>>,
    /// transactions stats
    stats: Mutex<TxStats>,
    /// List of callbacks that will be called after commit
    commit_handlers: Vec<Box<dyn Fn()>>,

    // WriteFlag specifies the flag for write-related methods like WriteTo().
    // Tx opens the database file with the specified flag to copy the data.
    //
    // By default, the flag is unset, which works well for mostly in-memory
    // workloads. For databases that are much larger than available RAM,
    // set the flag to syscall.O_DIRECT to avoid trashing the page cache.
    write_flag: usize,
}

pub struct Tx(Arc<RawTx>);

unsafe impl Sync for Tx {}

unsafe impl Send for Tx {}

#[derive(Debug, Clone)]
pub(crate) struct WeakTx(Weak<RawTx>);

impl WeakTx {
    pub(crate) fn new() -> Self {
        Self(Weak::new())
    }

    pub(crate) fn upgrade(&self) -> Option<Tx> {
        self.0.upgrade().map(Tx)
    }

    pub(crate) fn from(tx: &Tx) -> Self {
        Self(Arc::downgrade(&tx.0))
    }
}
#[derive(Debug)]
pub struct TxStats {
    // Page statistics.
    // #[deprecated(since = "future version", note = "Use GetPageCount() or IncPageCount() instead")]
    pub page_count: i64, // number of page allocations

    // #[deprecated(since = "future version", note = "Use GetPageAlloc() or IncPageAlloc() instead")]
    pub page_alloc: i64, // total bytes allocated

    // Cursor statistics.
    // #[deprecated(since = "future version", note = "Use GetCursorCount() or IncCursorCount() instead")]
    pub cursor_count: i64, // number of cursors created

    // Node statistics
    // #[deprecated(since = "future version", note = "Use GetNodeCount() or IncNodeCount() instead")]
    pub node_count: i64, // number of node allocations

    // #[deprecated(since = "future version", note = "Use GetNodeDeref() or IncNodeDeref() instead")]
    pub node_deref: i64, // number of node dereferences

    // Rebalance statistics.
    // #[deprecated(since = "future version", note = "Use GetRebalance() or IncRebalance() instead")]
    pub rebalance: i64, // number of node rebalances

    // #[deprecated(since = "future version", note = "Use GetRebalanceTime() or IncRebalanceTime() instead")]
    pub rebalance_time: std::time::Duration, // total time spent rebalancing

    // Split/Spill statistics.
    // #[deprecated(since = "future version", note = "Use GetSplit() or IncSplit() instead")]
    pub split: i64, // number of nodes split

    // #[deprecated(since = "future version", note = "Use GetSpill() or IncSpill() instead")]
    pub spill: i64, // number of nodes spilled

    // #[deprecated(since = "future version", note = "Use GetSpillTime() or IncSpillTime() instead")]
    pub spill_time: std::time::Duration, // total time spent spilling

    // Write statistics.
    // #[deprecated(since = "future version", note = "Use GetWrite() or IncWrite() instead")]
    pub write: i64, // number of writes performed

    // #[deprecated(since = "future version", note = "Use GetWriteTime() or IncWriteTime() instead")]
    pub write_time: std::time::Duration, // total time spent writing to disk
}
