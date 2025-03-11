use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Sub;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::Duration;

use crate::bucket::{BucketStructure, RawBucket, WeakBucket};
use crate::common::meta::Meta;
use crate::common::page::{OwnedPage, PageInfo, PgId};
use crate::common::types::Bytes;
use crate::cursor::Cursor;
use crate::db::WeakDB;
use crate::Bucket;

pub trait TxApi<'tx>: Clone + Send + Sync {
    /// ID returns the transaction id.
    fn id(&self) -> u64;

    // DB returns a reference to the database that created the transaction.
    fn db(&self) -> WeakDB<'tx>;

    // Size returns current database size in bytes as seen by this transaction.
    fn size(&self) -> u64;

    // Writable returns whether the transaction can perform write operations.
    fn writable(&self) -> bool;

    // Cursor creates a cursor associated with the root bucket.
    // All items in the cursor will return a nil value because all root bucket keys point to buckets.
    // The cursor is only valid as long as the transaction is open.
    // Do not use a cursor after the transaction is closed.
    fn cursor(&self) -> Cursor<'tx>;

    // Stats retrieves a copy of the current transaction statistics.
    fn stats(&self) -> TxStats;

    // Inspect returns the structure of the database.
    fn inspect() -> BucketStructure;

    // CreateBucket creates a new bucket.
    // Returns an error if the bucket already exists, if the bucket name is blank, or if the bucket name is too long.
    // The bucket instance is only valid for the lifetime of the transaction.
    fn create_bucket(&self, name: &str) -> crate::Result<Bucket<'tx>>;

    // CreateBucketIfNotExists creates a new bucket if it doesn't already exist.
    // Returns an error if the bucket name is blank, or if the bucket name is too long.
    // The bucket instance is only valid for the lifetime of the transaction.
    fn create_bucket_if_not_exists(&self, name: &str) -> crate::Result<Bucket<'tx>>;

    // DeleteBucket deletes a bucket.
    // Returns an error if the bucket cannot be found or if the key represents a non-bucket value.
    fn delete_bucket(&self, name: &str) -> crate::Result<()>;

    // MoveBucket moves a sub-bucket from the source bucket to the destination bucket.
    // Returns an error if
    //  1. the sub-bucket cannot be found in the source bucket;
    //  2. or the key already exists in the destination bucket;
    //  3. the key represents a non-bucket value.
    //
    // If src is nil, it means moving a top level bucket into the target bucket.
    // If dst is nil, it means converting the child bucket into a top level bucket.
    fn move_bucket(&self, child: &Bytes, src: &str, dst: &str) -> crate::Result<()>;

    // ForEach executes a function for each bucket in the root.
    // If the provided function returns an error then the iteration is stopped and
    // the error is returned to the caller.
    fn for_each<F>(&self, f: F) -> crate::Result<()>
    where
        F: FnMut(&Bytes, &Bucket<'tx>) -> crate::Result<()>;

    // OnCommit adds a handler function to be executed after the transaction successfully commits.
    fn on_commit<F>(&self, f: F)
    where
        F: Fn() + 'tx;

    // Commit writes all changes to disk, updates the meta page and closes the transaction.
    // Returns an error if a disk write error occurs, or if Commit is
    // called on a read-only transaction.
    fn commit(&self) -> crate::Result<()>;

    // Rollback closes the transaction and ignores all previous updates. Read-only
    // transactions must be rolled back and not committed.
    fn rollback(&self) -> crate::Result<()>;

    // Copy writes the entire database to a writer.
    // This function exists for backwards compatibility.
    //
    // Deprecated: Use WriteTo() instead.
    fn copy<W>(&self, w: W) -> crate::Result<()>
    where
        W: std::io::Write;

    // WriteTo writes the entire database to a writer.
    // If err == nil then exactly tx.Size() bytes will be written into the writer.
    fn write_to<W>(&self, w: W) -> crate::Result<()>
    where
        W: std::io::Write;

    // Page returns page information for a given page number.
    // This is only safe for concurrent use when used by a writable transaction.
    fn page(&self, id: PgId) -> crate::Result<PageInfo>;
}

pub(crate) struct TxCell<'tx> {
    raw: RefCell<RawTx<'tx>>,
}

pub struct Tx<'tx>(Arc<TxCell<'tx>>);

impl<'tx> Tx<'tx> {
    pub(crate) fn writable(&self) -> bool {
        self.0.raw.borrow().writable.load(Ordering::Relaxed)
    }

    pub(crate) fn new() -> Self {
        Self(todo!())
    }
}

unsafe impl<'tx> Sync for Tx<'tx> {}

unsafe impl<'tx> Send for Tx<'tx> {}

#[derive(Debug, Clone)]
pub(crate) struct WeakTx<'tx>(Weak<TxCell<'tx>>);

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

pub trait RawTxApi<'tx>: Clone + Send + Sync {
    // allocate returns a contiguous block of memory starting at a given page.
    fn allocate(self, count: usize) -> crate::Result<OwnedPage>;

    // write writes any dirty pages to disk.
    fn write(self) -> crate::Result<()>;

    // writeMeta writes the meta to the disk.
    fn write_meta(self) -> crate::Result<()>;

    // ForEachPage executes a function for each page that the transaction can see.
    // If the provided function returns an error then the iteration is stopped and
    // the error is returned to the caller.
    fn for_each_page<F>(self, f: F) -> crate::Result<()>
    where
        F: FnMut(&OwnedPage) -> crate::Result<()>;

    // ForEachNode executes a function for each node that the transaction can see.
    // If the provided function returns an error then the iteration is stopped and
    // the error is returned to the caller.
    fn for_each_node<F>(self, f: F) -> crate::Result<()>
    where
        F: FnMut(&Bytes, &Bytes) -> crate::Result<()>;

    // // ForEachLeaf executes a function for each leaf node that the transaction can see.
    // // If the provided function returns an error then the iteration is stopped and
    // // the error is returned to the caller.
    // fn for_each_leaf<F>(self, f: F) -> crate::Result<()>
    // where
    //     F: FnMut(&Bytes, &Bytes) -> crate::Result<()>;

    // // ForEachBucketNode executes a function for each bucket node that the transaction can see.
    // // If the provided function returns an error then the iteration is stopped and
    // // the error is returned to the caller.
    // fn for_each_bucket_node<F>(self, f: F) -> crate::Result<()>
    // where
    //     F: FnMut(&Bytes, &Bytes) -> crate::Result<()>;

    // // ForEachBucketLeaf executes a function for each bucket leaf node that the transaction can see.
    // // If the provided function returns an error then the iteration is stopped and
    // // the error is returned to the caller.
    // fn for_each_bucket_leaf<F>(self, f: F) -> crate::Result<()>
    // where
    //     F: FnMut(&Bytes, &Bytes) -> crate::Result<()>;
}

// RawTx represents a read-only or read/write transaction on the database.
// Read-only transactions can be used for retrieving values for keys and creating cursors.
// Read/write transactions can create and remove buckets and create and remove keys.
//
// IMPORTANT: You must commit or rollback transactions when you are done with
// them. Pages can not be reclaimed by the writer until no more transactions
// are using them. A long running read transaction can cause the database to
// quickly grow.
pub struct RawTx<'tx> {
    writable: AtomicBool,

    managed: AtomicBool,

    db: RwLock<WeakDB<'tx>>,
    /// transaction meta
    meta: RwLock<Meta>,
    /// root bucket
    root: RwLock<WeakBucket<'tx>>,
    /// cache page
    pages: RwLock<HashMap<PgId, OwnedPage>>,
    /// transactions stats
    stats: Option<Arc<TxStats>>,
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
