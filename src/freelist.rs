use crate::common::PgId;
use crate::common::PgIds;
use crate::common::TxId;
use crate::common::{self};
use std::collections::HashMap; // Assuming common.Pgid has Eq and Hash traits
use std::sync::{Arc, Mutex}; // Assuming common.Pgid has Eq and Hash traits

// type Pgids = Vec<PgId>;

// Represents a pending transaction with associated Pgids and allocation Txids
#[derive(Debug)]
struct TxPending {
    ids: Vec<PgId>,
    alloctx: Vec<TxId>,
    last_release_begin: TxId,
}

// Represents a set of Pgids with the same span size
type PidSet = HashMap<PgId, ()>; // Use an empty struct value for the set

// impl PidSet {
//     fn new() -> Self {
//         HashMap::new()
//     }

//     fn insert(&mut self, pid: common::page::PgId) {
//         self.insert(pid, ());
//     }

//     fn contains(&self, pid: &common::page::PgId) -> bool {
//         self.contains_key(pid)
//     }
// }

impl TxPending {
    // ... other methods for TxPending (if needed)
}

// txPending represents pending pages for a transaction.
// ReadWriter trait, similar to Go's interface
trait ReadWriter {
    // Read calls Init with the page ids stored in the given page.
    fn read(&mut self, page: &common::Page);

    // Write writes the freelist into the given page.
    fn write(&mut self, page: &mut common::Page);

    // EstimatedWritePageSize returns the size in bytes of the freelist after serialization in Write.
    // This should never underestimate the size.
    fn estimated_write_page_size(&self) -> usize;
}

// Interface trait, extending ReadWriter
trait Interface: ReadWriter {
    // Init initializes this freelist with the given list of pages.
    fn init(&mut self, ids: PgIds);

    // Allocate tries to allocate the given number of contiguous pages
    // from the free list pages. It returns the starting page ID if
    // available; otherwise, it returns 0.
    fn allocate(&mut self, txid: common::TxId, num_pages: usize) -> PgId;

    // Count returns the number of free and pending pages.
    fn count(&self) -> usize;

    // FreeCount returns the number of free pages.
    fn free_count(&self) -> usize;

    // PendingCount returns the number of pending pages.
    fn pending_count(&self) -> usize;
    // AddReadonlyTXID adds a given read-only transaction id for pending page tracking.
    fn add_readonly_txid(&mut self, txid: common::TxId);

    // RemoveReadonlyTXID removes a given read-only transaction id for pending page tracking.
    fn remove_readonly_txid(&mut self, txid: common::TxId);

    // ReleasePendingPages releases any pages associated with closed read-only transactions.
    fn release_pending_pages(&mut self);

    // Free releases a page and its overflow for a given transaction id.
    // If the page is already free or is one of the meta pages, then a panic will occur.
    fn free(&mut self, txid: common::TxId, page: &common::Page);

    // Freed returns whether a given page is in the free list.
    fn freed(&self, pgid: common::PgId) -> bool;

    // Rollback removes the pages from a given pending tx.
    fn rollback(&mut self, txid: common::TxId);

    // Copyall copies a list of all free ids and all pending ids in one sorted list.
    // f.count returns the minimum length required for dst.
    fn copy_all(&self, dst: &mut Vec<common::PgId>);

    // Reload reads the freelist from a page and filters out pending items.
    fn reload(&mut self, page: &common::Page);

    // NoSyncReload reads the freelist from Pgids and filters out pending items.
    fn no_sync_reload(&mut self, pgids: PgIds);

    // freePageIds returns the IDs of all free pages. Returns an empty slice if no free pages are available.
    fn free_page_ids(&self) -> PgIds;

    // pendingPageIds returns all pending pages by transaction id.
    fn pending_page_ids(&self) -> HashMap<common::TxId, TxPending>;

    // release moves all page ids for a transaction id (or older) to the freelist.
    fn release(&mut self, txid: common::TxId);

    // releaseRange moves pending pages allocated within an extent [begin,end] to the free list.
    fn release_range(&mut self, begin: common::TxId, end: common::TxId);

    // mergeSpans is merging the given pages into the freelist
    fn merge_spans(&mut self, ids: PgIds);
}

pub(crate) struct Freelist;
