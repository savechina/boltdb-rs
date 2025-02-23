use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Sub;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::Duration;

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

pub trait TxApi<'tx>: Clone + Send + Sync {}

pub struct RawTx<'tx> {
    writable: AtomicBool,
    managed: AtomicBool,
    db: RwLock<WeakDB<'tx>>,
    /// transaction meta
    meta: RwLock<Meta>,
    /// root bucket
    root: RwLock<Bucket<'tx>>,
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

pub struct Tx<'tx>(Arc<RawTx<'tx>>);
impl<'tx> Tx<'tx> {
    pub(crate) fn writable(&self) -> bool {
        self.0.writable.load(Ordering::Relaxed)
    }

    pub(crate) fn new() -> Self {
        Self(todo!())
    }
}

unsafe impl<'tx> Sync for Tx<'tx> {}

unsafe impl<'tx> Send for Tx<'tx> {}

#[derive(Debug, Clone)]
pub(crate) struct WeakTx<'tx>(Weak<RawTx<'tx>>);

impl<'tx> WeakTx<'tx> {
    pub(crate) fn new() -> Self {
        Self(Weak::new())
    }

    pub(crate) fn upgrade(&self) -> Option<Tx<'tx>> {
        self.0.upgrade().map(Tx)
    }

    pub(crate) fn from(tx: &Tx<'tx>) -> Self {
        Self(Arc::downgrade(&tx.0))
    }
}

#[derive(Debug, Default)]
pub struct TxStats {
    // Page statistics.
    // #[deprecated(since = "future version", note = "Use GetPageCount() or IncPageCount() instead")]
    page_count: AtomicI64, // number of page allocations

    // #[deprecated(since = "future version", note = "Use GetPageAlloc() or IncPageAlloc() instead")]
    page_alloc: AtomicI64, // total bytes allocated

    // Cursor statistics.
    // #[deprecated(since = "future version", note = "Use GetCursorCount() or IncCursorCount() instead")]
    cursor_count: AtomicI64, // number of cursors created

    // Node statistics
    // #[deprecated(since = "future version", note = "Use GetNodeCount() or IncNodeCount() instead")]
    node_count: AtomicI64, // number of node allocations

    // #[deprecated(since = "future version", note = "Use GetNodeDeref() or IncNodeDeref() instead")]
    node_deref: AtomicI64, // number of node dereferences

    // Rebalance statistics.
    // #[deprecated(since = "future version", note = "Use GetRebalance() or IncRebalance() instead")]
    rebalance: AtomicI64, // number of node rebalances

    // #[deprecated(since = "future version", note = "Use GetRebalanceTime() or IncRebalanceTime() instead")]
    rebalance_time: Duration, // total time spent rebalancing

    // Split/Spill statistics.
    // #[deprecated(since = "future version", note = "Use GetSplit() or IncSplit() instead")]
    split: AtomicI64, // number of nodes split

    // #[deprecated(since = "future version", note = "Use GetSpill() or IncSpill() instead")]
    spill: AtomicI64, // number of nodes spilled

    // #[deprecated(since = "future version", note = "Use GetSpillTime() or IncSpillTime() instead")]
    spill_time: Duration, // total time spent spilling

    // Write statistics.
    // #[deprecated(since = "future version", note = "Use GetWrite() or IncWrite() instead")]
    write: AtomicI64, // number of writes performed

    // #[deprecated(since = "future version", note = "Use GetWriteTime() or IncWriteTime() instead")]
    write_time: Duration, // total time spent writing to disk
}
impl TxStats {
    pub fn page_count(&self) -> i64 {
        self.page_count.load(Ordering::Acquire)
    }

    pub fn inc_page_count(&self) {
        self.page_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn page_alloc(&self) -> i64 {
        self.page_alloc.load(Ordering::Acquire)
    }

    pub fn inc_page_alloc(&self) {
        self.page_alloc.fetch_add(1, Ordering::Relaxed);
    }

    pub fn cursor_count(&self) -> i64 {
        self.cursor_count.load(Ordering::Acquire)
    }
    pub fn inc_cursor_count(&self) {
        self.cursor_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn spill(&self) -> i64 {
        self.spill.load(Ordering::Acquire)
    }
    pub fn inc_spill(&self) {
        self.spill.fetch_add(1, Ordering::Relaxed);
    }
    pub fn spill_time(&self) -> Duration {
        self.spill_time
    }
    pub fn inc_spill_time(&mut self, d: Duration) {
        self.spill_time += d;
    }
    pub fn write(&self) -> i64 {
        self.write.load(Ordering::Acquire)
    }
    pub fn inc_write(&self) {
        self.write.fetch_add(1, Ordering::Relaxed);
    }
    pub fn write_time(&self) -> Duration {
        self.write_time
    }
    pub fn node_count(&self) -> i64 {
        self.node_count.load(Ordering::Acquire)
    }
    pub fn inc_node_count(&self) {
        self.node_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn node_deref(&self) -> i64 {
        self.node_deref.load(Ordering::Acquire)
    }
    pub fn inc_node_deref(&self) {
        self.node_deref.fetch_add(1, Ordering::Relaxed);
    }
    pub fn rebalance(&self) -> i64 {
        self.rebalance.load(Ordering::Acquire)
    }
    pub fn inc_rebalance(&self) {
        self.rebalance.fetch_add(1, Ordering::Relaxed);
    }
    pub fn rebalance_time(&self) -> Duration {
        self.rebalance_time
    }
    pub fn inc_rebalance_time(&self, d: Duration) {
        // self.rebalance_time += d;
    }
    pub fn split(&self) -> i64 {
        self.split.load(Ordering::Acquire)
    }
    pub fn inc_split(&self) {
        self.split.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn sub(&self, other: &TxStats) -> TxStats {
        todo!()
    }
}

impl Clone for TxStats {
    fn clone(&self) -> Self {
        Self {
            spill: AtomicI64::new(self.spill.load(Ordering::Acquire)),
            spill_time: self.spill_time,
            write: AtomicI64::new(self.write.load(Ordering::Acquire)),
            write_time: self.write_time,
            page_count: AtomicI64::new(self.page_count.load(Ordering::Acquire)),
            page_alloc: AtomicI64::new(self.page_alloc.load(Ordering::Acquire)),
            cursor_count: AtomicI64::new(self.cursor_count.load(Ordering::Acquire)),
            node_count: AtomicI64::new(self.node_count.load(Ordering::Acquire)),
            node_deref: AtomicI64::new(self.node_deref.load(Ordering::Acquire)),
            rebalance: AtomicI64::new(self.rebalance.load(Ordering::Acquire)),
            rebalance_time: self.rebalance_time,
            split: AtomicI64::new(self.split.load(Ordering::Acquire)),
        }
    }
}
