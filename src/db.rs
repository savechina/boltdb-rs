use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::path::Path;
use std::ptr::NonNull;
use std::sync::atomic::AtomicI64;
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};
use std::time::Duration;

use crate::common::TxId;
use crate::common::meta::Meta;
use crate::common::types::DEFAULT_PAGE_SIZE;
use crate::errors::Result;
use crate::freelist::Freelist;
use crate::tx::{Tx, TxApi, TxStats};

// FreelistType is the type of the freelist backend
// FreelistType represents the type of freelist used by the database.

// TODO(ahrtr): eventually we should (step by step)
//  1. default to `FreelistMapType`;
//  2. remove the `FreelistArrayType`, do not export `FreelistMapType`
//     and remove field `FreelistType' from both `DB` and `Options`;
#[derive(Debug, PartialEq, Clone, Copy)]
enum FreelistType {
    // FreelistArrayType indicates backend freelist type is array
    Array,
    // FreelistMapType indicates backend freelist type is hashmap
    HashMap,
}

impl Display for FreelistType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            FreelistType::Array => write!(f, "array"),
            FreelistType::HashMap => write!(f, "hashmap"),
        }
    }
}

pub trait DbApi: Clone + Send + Sync
where
    Self: Sized,
{
    // Path returns the path to currently open database file.
    fn path(&self) -> String;

    // Begin starts a new transaction.
    // Multiple read-only transactions can be used concurrently but only one
    // write transaction can be used at a time. Starting multiple write transactions
    // will cause the calls to block and be serialized until the current write
    // transaction finishes.
    //
    // Transactions should not be dependent on one another. Opening a read
    // transaction and a write transaction in the same goroutine can cause the
    // writer to deadlock because the database periodically needs to re-mmap itself
    // as it grows and it cannot do that while a read transaction is open.
    //
    // If a long running read transaction (for example, a snapshot transaction) is
    // needed, you might want to set DB.InitialMmapSize to a large enough value
    // to avoid potential blocking of write transaction.
    //
    // IMPORTANT: You must close read-only transactions after you are finished or
    // else the database will not reclaim old pages.
    // fn begin(&self) -> crate::Result<impl TxApi>;

    // Begin starts a new transaction.
    // Multiple read-only transactions can be used concurrently but only one
    // write transaction can be used at a time. Starting multiple write transactions
    // will cause the calls to block and be serialized until the current write
    // transaction finishes.
    //
    // Transactions should not be dependent on one another. Opening a read
    // transaction and a write transaction in the same goroutine can cause the
    // writer to deadlock because the database periodically needs to re-mmap itself
    // as it grows and it cannot do that while a read transaction is open.
    //
    // If a long running read transaction (for example, a snapshot transaction) is
    // needed, you might want to set DB.InitialMmapSize to a large enough value
    // to avoid potential blocking of write transaction.
    //
    // IMPORTANT: You must close read-only transactions after you are finished or
    // else the database will not reclaim old pages.
    // fn begin_rw(&mut self) -> crate::Result<impl TxApi>;

    // View executes a function within the context of a managed read-only transaction.
    // Any error that is returned from the function is returned from the View() method.
    //
    // Attempting to manually rollback within the function will cause a panic.
    fn view<'tx, Fn>(&'tx self, f: Fn) -> crate::Result<()>
    where
        Fn: FnMut(Tx<'tx>) -> crate::Result<()>;

    // Update executes a function within the context of a read-write managed transaction.
    // If no error is returned from the function then the transaction is committed.
    // If an error is returned then the entire transaction is rolled back.
    // Any error that is returned from the function or returned from the commit is
    // returned from the Update() method.
    //
    // Attempting to manually commit or rollback within the function will cause a panic.
    fn update<'tx, Fn>(&'tx mut self, f: Fn) -> crate::Result<()>
    where
        Fn: FnMut(Tx<'tx>) -> crate::Result<()>;

    // Batch calls fn as part of a batch. It behaves similar to Update,
    // except:
    //
    // 1. concurrent Batch calls can be combined into a single Bolt
    // transaction.
    //
    // 2. the function passed to Batch may be called multiple times,
    // regardless of whether it returns error or not.
    //
    // This means that Batch function side effects must be idempotent and
    // take permanent effect only after a successful return is seen in
    // caller.
    //
    // The maximum batch size and delay can be adjusted with DB.MaxBatchSize
    // and DB.MaxBatchDelay, respectively.
    //
    // Batch is only useful when there are multiple goroutines calling it.
    fn batch<'tx, Handler>(&'tx mut self, handler: Handler) -> crate::Result<()>
    where
        Handler: FnMut(Tx<'tx>) -> crate::Result<()> + Send + Sync + Clone + 'static;

    // Close releases all database resources.
    // It will block waiting for any open transactions to finish
    // before closing the database and returning.
    fn close(self) -> crate::Result<()>;

    // Sync executes fdatasync() against the database file handle.
    //
    // This is not necessary under normal operation, however, if you use NoSync
    // then it allows you to force the database file to sync against the disk.

    fn sync(&mut self) -> crate::Result<()>;

    // Stats retrieves ongoing performance stats for the database.
    // This is only updated when a transaction closes.
    fn stats(&self) -> Arc<Stats>;

    // This is for internal access to the raw data bytes from the C cursor, use
    // carefully, or not at all.
    fn info(&self) -> Info;
}

// DB represents a collection of buckets persisted to a file on disk.
// All data access is performed through transactions which can be obtained through the DB.
// All the functions on DB will return a ErrDatabaseNotOpen if accessed before Open() is called.
pub(crate) struct RawDB {
    // Put `stats` at the first field to ensure it's 64-bit aligned. Note that
    // the first word in an allocated struct can be relied upon to be 64-bit
    // aligned. Refer to https://pkg.go.dev/sync/atomic#pkg-note-BUG. Also
    // refer to discussion in https://github.com/etcd-io/bbolt/issues/577.
    stats: Arc<Mutex<Stats>>, // Thread-safe access to statistics

    // When enabled, the database will perform a Check() after every commit.
    // A panic is issued if the database is in an inconsistent state. This
    // flag has a large performance impact so it should only be used for
    // debugging purposes.
    // Flags with explicit defaults
    strict_mode: bool,

    // Setting the NoSync flag will cause the database to skip fsync()
    // calls after each commit. This can be useful when bulk loading data
    // into a database and you can restart the bulk load in the event of
    // a system failure or database corruption. Do not set this flag for
    // normal use.
    //
    // If the package global IgnoreNoSync constant is true, this value is
    // ignored.  See the comment on that constant for more details.
    //
    // THIS IS UNSAFE. PLEASE USE WITH CAUTION.
    no_sync: bool,

    // When true, skips syncing freelist to disk. This improves the database
    // write performance under normal operation, but requires a full database
    // re-sync during recovery.
    no_freelist_sync: bool,

    // FreelistType sets the backend freelist type. There are two options. Array which is simple but endures
    // dramatic performance degradation if database is large and fragmentation in freelist is common.
    // The alternative one is using hashmap, it is faster in almost all circumstances
    // but it doesn't guarantee that it offers the smallest page id available. In normal case it is safe.
    // The default type is array
    freelist_type: FreelistType,

    // When true, skips the truncate call when growing the database.
    // Setting this to true is only safe on non-ext3/ext4 systems.
    // Skipping truncation avoids preallocation of hard drive space and
    // bypasses a truncate() and fsync() syscall on remapping.
    //
    // https://github.com/boltdb/bolt/issues/284
    no_grow_sync: bool,

    // When `true`, bbolt will always load the free pages when opening the DB.
    // When opening db in write mode, this flag will always automatically
    // set to `true`.
    pre_load_freelist: bool,

    // If you want to read the entire database fast, you can set MmapFlag to
    // syscall.MAP_POPULATE on Linux 2.6.23+ for sequential read-ahead.
    mmap_flags: i32,

    // Configuration options
    // MaxBatchSize is the maximum size of a batch. Default value is
    // copied from DefaultMaxBatchSize in Open.
    //
    // If <=0, disables batching.
    //
    // Do not change concurrently with calls to Batch.
    max_batch_size: isize,

    // MaxBatchDelay is the maximum delay before a batch starts.
    // Default value is copied from DefaultMaxBatchDelay in Open.
    //
    // If <=0, effectively disables batching.
    //
    // Do not change concurrently with calls to Batch.
    max_batch_delay: Duration,

    // AllocSize is the amount of space allocated when the database
    // needs to create new pages. This is done to amortize the cost
    // of truncate() and fsync() when growing the data file.
    alloc_size: usize,

    // Mlock locks database file in memory when set to true.
    // It prevents major page faults, however used memory can't be reclaimed.
    //
    // Supported only on Unix via mlock/munlock syscalls.
    mlock: bool,

    // logger: Option<Logger>, // Optional logger
    path: String,
    file: Option<Arc<Mutex<File>>>, // Thread-safe file handle

    // `dataref` isn't used at all on Windows, and the golangci-lint
    // always fails on Windows platform.
    //nolint
    dataref: Option<Vec<u8>>,        // mmap'ed readonly, write throws SEGV
    data: Option<Box<[u8]>>,         // Optional data pointer (writeable)
    datasz: usize,                   // Current data length
    meta0: Option<Arc<Mutex<Meta>>>, // Thread-safe meta page 0
    meta1: Option<Arc<Mutex<Meta>>>, // Thread-safe meta page 1
    page_size: usize,                // Page size for the database
    opened: bool,                    // Whether the database is open or not
    rwtx: Option<TxId>,              // Read-write transaction (writer)
    txs: Vec<TxId>,                  // Read-only transactions

    freelist: Option<Arc<Mutex<Freelist>>>, // Thread-safe freelist access
    freelist_load: OnceLock<bool>,          // Flag to track freelist loading

    page_pool: Mutex<Vec<Box<[u8]>>>, // Pool of allocated pages

    batch_mu: Mutex<Option<Batch>>, // Mutex for batch operations

    rwlock: Mutex<()>,    // Allows only one writer at a time.
    metalock: Mutex<()>,  // Protects meta page access.
    mmaplock: RwLock<()>, // Protects mmap access during remapping.
    statlock: RwLock<()>, // Protects stats access.

    ops: Ops, // Operations struct for file access

    read_only: bool, // Read-only mode flag
}

impl Default for RawDB {
    fn default() -> Self {
        RawDB {
            stats: Arc::new(Mutex::new(Stats::new())),
            strict_mode: false,
            no_sync: false,
            no_freelist_sync: false,
            freelist_type: FreelistType::Array,
            no_grow_sync: false,
            pre_load_freelist: false,
            mmap_flags: 0,
            max_batch_size: 0,
            max_batch_delay: Duration::from_millis(0),
            alloc_size: 0,
            mlock: false,
            page_size: *DEFAULT_PAGE_SIZE,
            page_pool: Mutex::new(Vec::new()),
            rwlock: Mutex::new(()),
            path: String::from("test.db"),
            file: None,
            dataref: None,
            data: None,
            datasz: 2048,
            meta0: Some(Arc::new(Mutex::new(Meta::default()))),
            meta1: Some(Arc::new(Mutex::new(Meta::default()))),
            opened: true,
            rwtx: None,
            txs: vec![],
            freelist: None,
            freelist_load: OnceLock::new(),
            batch_mu: Mutex::new(None),
            metalock: Mutex::new(()),
            mmaplock: RwLock::new(()),
            statlock: RwLock::new(()),
            ops: Ops {
                write_at: |_buf: &[u8], _off: i64| Ok(0),
            },
            read_only: false,
        }
    }
}

unsafe impl Send for RawDB {}
unsafe impl Sync for RawDB {}

impl RawDB {}

struct Ops {
    write_at: fn(&[u8], i64) -> Result<usize>,
}

// DB represents a collection of buckets persisted to a file on disk.
// All data access is performed through transactions which can be obtained through the DB.
// All the functions on DB will return a ErrDatabaseNotOpen if accessed before Open() is called.
#[derive(Clone)]
pub struct DB(pub(crate) Arc<RawDB>);

impl DB {
    pub(crate) fn begin_tx(&self) -> crate::Result<Tx> {
        let raw_db = self.0.clone();
        let tx = Tx::new();
        Ok(tx)
    }
}

impl DB {
    /// Open creates and opens a database at the given path.
    /// If the file does not exist then it will be created automatically.
    pub fn open<T: AsRef<Path>>(path: T) -> crate::Result<Self> {
        DB::open_with(path, Options::default())
    }

    /// Open creates and opens a database at the given path.
    /// If the file does not exist then it will be created automatically.
    pub fn open_with<T: AsRef<Path>>(path: T, options: Options) -> crate::Result<Self> {
        // DB::open_path(path, Options::default())
        Ok(DB(Arc::new(RawDB::default())))
    }

    pub fn path(&self) -> String {
        self.0.path.clone()
    }
}

impl DbApi for DB {
    // fn begin_tx(&self) -> crate::Result<impl TxApi> {
    //     self.begin_tx()
    // }
    fn path(&self) -> String {
        self.path()
    }

    // fn begin(&self) -> crate::Result<impl TxApi> {
    //     Ok(())
    // }

    // fn begin_rw(&mut self) -> crate::Result<impl TxApi> {
    //     todo!()
    // }

    fn view<'tx, Fn>(&'tx self, mut f: Fn) -> crate::Result<()>
    where
        Fn: FnMut(Tx<'tx>) -> crate::Result<()>,
    {
        let tx = self.begin_tx()?;

        let result = f(tx);
        result
    }

    fn update<'tx, Handler>(&'tx mut self, mut f: Handler) -> crate::Result<()>
    where
        Handler: FnMut(Tx<'tx>) -> crate::Result<()>,
    {
        let tx = self.begin_tx()?;

        let result = f(tx);

        match result {
            Ok(_) => {
                // commit tx
                // tx.commit()?;
                Ok(())
            }
            Err(e) => {
                // rollback tx
                // tx.rollback()?;
                Err(e)
            }
        }
    }

    fn batch<'tx, Handler>(&'tx mut self, mut handler: Handler) -> crate::Result<()>
    where
        Handler: FnMut(Tx<'tx>) -> crate::Result<()> + Send + Sync + Clone + 'static,
    {
        todo!()
    }

    fn close(self) -> crate::Result<()> {
        // unimplemented!()
        todo!()
    }

    fn sync(&mut self) -> crate::Result<()> {
        todo!()
    }

    fn stats(&self) -> Arc<Stats> {
        todo!()
    }

    fn info(&self) -> Info {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WeakDB(Weak<RawDB>);

impl<'tx> WeakDB {
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

// Logger is the logger trait used by bbolt.
trait Logger {
    fn log(&self, message: String);
}

// Options represents the options that can be set when opening a database.
#[derive(Debug, Clone)]
pub struct Options {
    // Timeout is the amount of time to wait to obtain a file lock.
    // When set to zero it will wait indefinitely.
    timeout: Duration,

    // Sets the DB.NoGrowSync flag before memory mapping the file.
    no_grow_sync: bool,

    // Do not sync freelist to disk. This improves the database write performance
    // under normal operation, but requires a full database re-sync during recovery.
    no_freelist_sync: bool,

    // PreLoadFreelist sets whether to load the free pages when opening
    // the db file. Note when opening db in write mode, bbolt will always
    // load the free pages.
    pre_load_freelist: bool,

    // FreelistType sets the backend freelist type. There are two options. Array which is simple but endures
    // dramatic performance degradation if database is large and fragmentation in freelist is common.
    // The alternative one is using hashmap, it is faster in almost all circumstances
    // but it doesn't guarantee that it offers the smallest page id available. In normal case it is safe.
    // The default type is array
    freelist_type: FreelistType,

    // Open database in read-only mode. Uses flock(..., LOCK_SH |LOCK_NB) to
    // grab a shared lock (UNIX).
    read_only: bool,

    // Sets the DB.MmapFlags flag before memory mapping the file.
    mmap_flags: i32,

    // InitialMmapSize is the initial mmap size of the database
    // in bytes. Read transactions won't block write transaction
    // if the InitialMmapSize is large enough to hold database mmap
    // size. (See DB.Begin for more information)
    //
    // If <=0, the initial map size is 0.
    // If initialMmapSize is smaller than the previous database size,
    // it takes no effect.
    initial_mmap_size: u64,

    // PageSize overrides the default OS page size.
    page_size: usize,

    // NoSync sets the initial value of DB.NoSync. Normally this can just be
    // set directly on the DB itself when returned from Open(), but this option
    // is useful in APIs which expose Options but not the underlying DB.
    no_sync: bool,

    // OpenFile is used to open files. It defaults to os::fs::OpenOptions. This option
    // is useful for writing hermetic tests.
    open_file: Option<fn(&str, i32, u32) -> crate::Result<File>>,

    // Mlock locks database file in memory when set to true.
    // It prevents potential page faults, however
    // used memory can't be reclaimed. (UNIX only)
    mlock: bool,
    // Logger is the logger used for bbolt.
    // logger: Option<Box<dyn Logger>>,
}

impl Options {
    fn to_string(&self) -> String {
        format!(
            "{{Timeout: {:?}, NoGrowSync: {}, NoFreelistSync: {}, PreLoadFreelist: {}, FreelistType: {}, ReadOnly: {}, MmapFlags: {:x}, InitialMmapSize: {}, PageSize: {}, NoSync: {}, OpenFile: {:?}, Mlock: {}, }}",
            self.timeout,
            self.no_grow_sync,
            self.no_freelist_sync,
            self.pre_load_freelist,
            self.freelist_type,
            self.read_only,
            self.mmap_flags,
            self.initial_mmap_size,
            self.page_size,
            self.no_sync,
            self.open_file.map(|f| f as *const ()),
            self.mlock,
            // self.logger.as_ref().map(|l| l as *const dyn Logger)
        )
    }
}

impl Default for Options {
    fn default() -> Self {
        Options {
            timeout: Duration::from_secs(30),
            no_grow_sync: false,
            no_freelist_sync: false,
            pre_load_freelist: false,
            freelist_type: FreelistType::Array,
            read_only: false,
            mmap_flags: 0,
            initial_mmap_size: 0,
            page_size: *DEFAULT_PAGE_SIZE,
            no_sync: false,
            open_file: None,
            mlock: false,
            // logger: None,
        }
    }
}

struct Batch;

// Stats represents statistics about the database.
#[derive(Default)]
struct Stats {
    // Put `TxStats` at the first field to ensure it's 64-bit aligned. Note
    // that the first word in an allocated struct can be relied upon to be
    // 64-bit aligned. Refer to https://pkg.go.dev/sync/atomic#pkg-note-BUG.
    // Also refer to discussion in https://github.com/etcd-io/bbolt/issues/577.
    tx_stats: TxStats, // global, ongoing stats.

    // Freelist stats
    free_page_n: i64,    // total number of free pages on the freelist
    pending_page_n: i64, // total number of pending pages on the freelist
    free_alloc: i64,     // total bytes allocated in free pages
    freelist_inuse: i64, // total bytes used by the freelist

    // Transaction stats
    tx_n: i64,      // total number of started read transactions
    open_tx_n: i64, // number of currently open read transactions
}

impl Stats {
    fn new() -> Self {
        Stats {
            tx_stats: TxStats::default(),
            free_page_n: 0,
            pending_page_n: 0,
            free_alloc: 0,
            freelist_inuse: 0,
            tx_n: 0,
            open_tx_n: 0,
        }
    }
    //getters for stats
    pub fn tx_stats(&self) -> &TxStats {
        &self.tx_stats
    }
    pub fn free_page_n(&self) -> i64 {
        self.free_page_n
    }
    pub fn pending_page_n(&self) -> i64 {
        self.pending_page_n
    }
    pub fn free_alloc(&self) -> i64 {
        self.free_alloc
    }

    pub fn freelist_inuse(&self) -> i64 {
        self.freelist_inuse
    }
    pub fn tx_n(&self) -> i64 {
        self.tx_n
    }
    pub fn open_tx_n(&self) -> i64 {
        self.open_tx_n
    }

    // setter for stats
    pub fn set_tx_stats(&mut self, tx_stats: TxStats) {
        self.tx_stats = tx_stats;
    }
    pub fn set_free_page_n(&mut self, free_page_n: i64) {
        self.free_page_n = free_page_n;
    }
    pub fn set_pending_page_n(&mut self, pending_page_n: i64) {
        self.pending_page_n = pending_page_n;
    }
    pub fn set_free_alloc(&mut self, free_alloc: i64) {
        self.free_alloc = free_alloc;
    }
    pub fn set_freelist_inuse(&mut self, freelist_inuse: i64) {
        self.freelist_inuse = freelist_inuse;
    }
    pub fn set_tx_n(&mut self, tx_n: i64) {
        self.tx_n = tx_n;
    }
    pub fn set_open_tx_n(&mut self, open_tx_n: i64) {
        self.open_tx_n = open_tx_n;
    }

    // Sub calculates and returns the difference between two sets of database stats.
    // This is useful when obtaining stats at two different points and time and
    // you need the performance counters that occurred within that time span.
    fn sub(&self, other: Option<&Stats>) -> Stats {
        if other.is_none() {
            return self.clone(); // Return a copy of self if other is None
        }

        let other = other.unwrap();

        Stats {
            tx_stats: self.tx_stats.sub(&other.tx_stats),
            free_page_n: self.free_page_n,
            pending_page_n: self.pending_page_n,
            free_alloc: self.free_alloc,
            freelist_inuse: self.freelist_inuse,
            tx_n: self.tx_n - other.tx_n,
            open_tx_n: self.open_tx_n,
        }
    }
}

impl Clone for Stats {
    fn clone(&self) -> Self {
        Stats {
            tx_stats: self.tx_stats.clone(),
            free_page_n: self.free_page_n,
            pending_page_n: self.pending_page_n,
            free_alloc: self.free_alloc,
            freelist_inuse: self.freelist_inuse,
            tx_n: self.tx_n,
            open_tx_n: self.open_tx_n,
        }
    }
}

struct Info {
    data: NonNull<u8>, // 使用 NonNull<u8> 替换 *const u8
    page_size: usize,
}

#[cfg(test)]
mod tests {
    use crate::testing::TestDb;
    use crate::{Error, Result};

    use super::*;

    #[test]
    fn test_db_size() {
        println!("RawDB size: {} bytes", std::mem::size_of::<RawDB>());
        // 同时也看看 Stats 及其它主要组件的大小
        println!("Stats size: {} bytes", std::mem::size_of::<Stats>());

        println!("DB size: {} bytes", std::mem::size_of::<DB>());

        println!("Options size: {} bytes", std::mem::size_of::<Options>());
    }

    #[test]
    fn test_db_create() {
        println!("RawDB size: {} bytes", std::mem::size_of::<RawDB>());

        let db = DB(Arc::new(RawDB {
            stats: Arc::new(Mutex::new(Stats::new())),
            strict_mode: false,
            no_sync: false,
            no_freelist_sync: false,
            freelist_type: FreelistType::Array,
            no_grow_sync: false,
            pre_load_freelist: false,
            mmap_flags: 0,
            max_batch_size: 0,
            max_batch_delay: Duration::from_secs(0),
            alloc_size: 0,
            mlock: false,
            page_size: 4096,
            page_pool: Mutex::new(Vec::new()),
            rwlock: Mutex::new(()),
            path: String::from("test.db"),
            file: None,
            dataref: None,
            data: None,
            datasz: 2048,
            meta0: Some(Arc::new(Mutex::new(Meta::default()))),
            meta1: Some(Arc::new(Mutex::new(Meta::default()))),
            opened: true,
            rwtx: None,
            txs: vec![],
            freelist: None,
            freelist_load: OnceLock::new(),
            batch_mu: Mutex::new(None),
            metalock: Mutex::new(()),
            mmaplock: RwLock::new(()),
            statlock: RwLock::new(()),
            ops: Ops {
                write_at: |_buf: &[u8], _off: i64| Ok(0),
            },
            read_only: false,
            // ..Default::default()
        }));

        let tx = db.begin_tx().unwrap();

        println!("Transaction created: {:?}", tx.writable());
        // assert_eq!(tx.writable(), false);
    }

    #[test]
    fn test_open() -> Result<()> {
        let db = TestDb::new()?;

        // db.clone_db().close()?;
        Ok(())
    }

    #[test]
    fn test_view() -> Result<()> {
        let db = TestDb::new()?;

        db.view(|tx| {
            println!("Inside view transaction, writable: {}", tx.writable());
            Ok(())
        })?;

        Ok(())
    }

    #[test]
    fn test_view_error() -> Result<()> {
        let db = TestDb::new()?;

        let result = db.view(|tx| Err(Error::Invalid)).err();
        assert_eq!(Some(Error::Invalid), result);
        Ok(())
    }

    #[test]
    fn test_db_info() {
        let data = String::from("test db info");

        let len = data.len();

        println!("Data length: {}", len);

        let data_ptr: *mut u8 = data.as_ptr() as *mut u8; // 示例内存地址
        let non_null_ptr = NonNull::new(data_ptr);
        assert!(non_null_ptr.is_some(), "NonNull pointer should not be null");

        if let Some(ptr) = non_null_ptr {
            let info = Info {
                data: ptr,
                page_size: 4096,
            };

            // 安全地使用 NonNull 指针（仍然需要 unsafe 块）
            unsafe {
                let value: u8 = *info.data.as_ptr();
                println!("Data value: {:?}", value);
            }

            println!("Page size: {}", info.page_size);
        } else {
            println!("Data pointer is null");
        }
    }
}
