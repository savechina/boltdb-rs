use crate::common::bucket::InBucket;
use crate::common::page::{OwnedPage, Page, PgId};
use crate::common::types::{self, Bytes};
use crate::cursor::Cursor;
use crate::errors::{Error, Result};
use crate::node::Node;
use crate::tx::{self, Tx, WeakTx};
use types::Key;
// use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem;
use std::ops::AddAssign;
use std::sync::Arc;
use std::sync::Weak;

// MaxKeySize is the maximum length of a key, in bytes.
const MAX_KEY_SIZE: usize = 32768;

// MaxValueSize is the maximum length of a value, in bytes.
const MAX_VALUE_SIZE: usize = (1 << 31) - 2;

const BUCKET_HEADER_SIZE: usize = mem::size_of::<RawBucket>();

pub(crate) const MIN_FILL_PERCENT: f64 = 0.1;
pub(crate) const MAX_FILL_PERCENT: f64 = 1.0;

/// DefaultFillPercent is the percentage that split pages are filled.
/// This value can be changed by setting Bucket.FillPercent.
pub(crate) const DEFAULT_FILL_PERCENT: f64 = 0.5;

pub trait BucketApi<'tx> {
    /// Tx returns the tx of the bucket.
    fn tx(self) -> Result<Tx<'tx>>;

    /// Root returns the root of the bucket.
    fn root(&self) -> PgId;

    /// Writable returns whether the bucket is writable.
    fn writeable(&self) -> bool;

    /// Cursor creates a cursor associated with the bucket.
    /// The cursor is only valid as long as the transaction is open.
    /// Do not use a cursor after the transaction is closed.
    fn cursor(self) -> Result<Cursor<'tx>>;

    // Bucket retrieves a nested bucket by name.
    // Returns nil if the bucket does not exist.
    // The bucket instance is only valid for the lifetime of the transaction.
    fn bucket(self, name: &Bytes) -> Result<Bucket<'tx>>;

    // Bucket retrieves a nested bucket by name.
    // Returns nil if the bucket does not exist.
    // The bucket instance is only valid for the lifetime of the transaction.
    fn bucket_mut(&self, name: &Bytes) -> Result<Bucket<'tx>>;

    // CreateBucket creates a new bucket at the given key and returns the new bucket.
    // Returns an error if the key already exists, if the bucket name is blank, or if the bucket name is too long.
    // The bucket instance is only valid for the lifetime of the transaction.
    fn create_bucket(&mut self, name: &Bytes) -> Result<Bucket<'tx>>;

    // CreateBucketIfNotExists creates a new bucket if it doesn't already exist and returns a reference to it.
    // Returns an error if the bucket name is blank, or if the bucket name is too long.
    // The bucket instance is only valid for the lifetime of the transaction.
    fn create_bucket_if_not_exists(&mut self, name: &Bytes) -> Result<Bucket<'tx>>;

    // DeleteBucket deletes a bucket at the given key.
    // Returns an error if the bucket does not exist, or if the key represents a non-bucket value.
    fn delete_bucket(&mut self, name: &Bytes) -> Result<()>;

    // MoveBucket moves a sub-bucket from the source bucket to the destination bucket.
    // Returns an error if
    //  1. the sub-bucket cannot be found in the source bucket;
    //  2. or the key already exists in the destination bucket;
    //  3. or the key represents a non-bucket value;
    //  4. the source and destination buckets are the same.
    fn move_bucket(&mut self, name: &Bytes, to: &Bucket<'tx>) -> Result<()>;

    /// Inspect returns the structure of the bucket.
    fn inspect(self) -> Result<BucketStructure>;

    fn page_node(&self, root_page: PgId) -> (&Page, &Node);

    fn root_page(self) -> PgId;

    // Get retrieves the value for a key in the bucket.
    // Returns a nil value if the key does not exist or if the key is a nested bucket.
    // The returned value is only valid for the life of the transaction.
    // The returned memory is owned by bbolt and must never be modified; writing to this memory might corrupt the database.
    fn get(&self, key: &Bytes) -> Option<&Bytes>;

    // Put sets the value for a key in the bucket.
    // If the key exist then its previous value will be overwritten.
    // Supplied value must remain valid for the life of the transaction.
    // Returns an error if the bucket was created from a read-only transaction, if the key is blank, if the key is too large, or if the value is too large.
    fn put(&mut self, key: &Bytes, value: &Bytes) -> Result<()>;

    // Delete removes a key from the bucket.
    // If the key does not exist then nothing is done and a nil error is returned.
    // Returns an error if the bucket was created from a read-only transaction.
    fn delete(&mut self, key: &Bytes) -> Result<()>;

    /// Sequence returns the current integer for the bucket without incrementing it.
    fn sequence(&self) -> Result<u64>;

    // SetSequence updates the sequence number for the bucket.
    // Returns an error if the bucket was created from a read-only transaction.
    fn set_sequence(&mut self, value: u64) -> Result<()>;

    // NextSequence returns an autoincrementing integer for the bucket.
    // Returns an error if the bucket was created from a read-only transaction.
    fn next_sequence(&mut self) -> Result<u64>;

    // ForEach executes a function for each key/value pair in a bucket.
    // Because ForEach uses a Cursor, the iteration over keys is in lexicographical order.
    // If the provided function returns an error then the iteration is stopped and
    // the error is returned to the caller. The provided function must not modify
    // the bucket; this will result in undefined behavior.
    fn for_each<F>(&self, f: F) -> Result<()>
    where
        F: FnMut(&Bytes, &Bytes) -> Result<()>;

    // ForEach executes a function for each bucket in a bucket.
    // If the provided function returns an error then the iteration is stopped and
    // the error is returned to the caller. The provided function must not modify
    // the bucket; this will result in undefined behavior.
    fn for_each_bucket<F>(&self, f: F) -> Result<()>
    where
        F: FnMut(&Bytes, &Bytes) -> Result<()>;

    // Stats returns stats on a bucket.
    fn stats(self) -> Result<BucketStats>;

    fn structure(self) -> Result<BucketStructure>;
}

#[derive(Debug, Clone)]
pub struct BucketCell<'tx> {
    raw: RefCell<RawBucket<'tx>>,
}

#[derive(Debug, Clone)]
pub struct Bucket<'tx>(Arc<BucketCell<'tx>>);

#[derive(Debug, Clone)]
pub(crate) struct WeakBucket<'tx>(Weak<BucketCell<'tx>>);

impl<'tx> WeakBucket<'tx> {
    pub(crate) fn new() -> Self {
        Self(Weak::new())
    }

    pub(crate) fn upgrade(&self) -> Option<Bucket<'tx>> {
        // self.0.upgrade().map(|bucket| Bucket(Arc::clone(&bucket)))
        self.0.upgrade().map(Bucket)
    }

    pub(crate) fn from(bucket: &Bucket<'tx>) -> Self {
        Self(Arc::downgrade(&bucket.0))
    }
}

// Bucket represents a collection of key/value pairs inside the database.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct RawBucket<'tx> {
    pub(crate) bucket: InBucket,
    // the associated transaction, WeakTx
    pub(crate) tx: WeakTx<'tx>,
    // subbucket cache
    pub(crate) buckets: RefCell<HashMap<Key, WeakBucket<'tx>>>,
    // inline page reference
    pub(crate) page: Option<OwnedPage>,
    // materialized node for the root page
    pub(crate) root_node: Option<Node<'tx>>,
    // node cache
    // TODO: maybe use refHashMap
    pub(crate) nodes: RefCell<HashMap<PgId, Node<'tx>>>,
    // Sets the threshold for filling nodes when they split. By default,
    // the bucket will fill to 50% but it can be useful to increase this
    // amount if you know that your write workloads are mostly append-only.
    //
    // This is non-persisted across transactions so it must be set in every Tx.
    pub(crate) fill_percent: f64,
}

impl<'tx> RawBucket<'tx> {
    pub(crate) fn node(&self, child_pgid: PgId, from: crate::node::WeakNode) -> Node {
        todo!()
    }

    // Tx returns the tx of the bucket.
    pub(crate) fn tx(&self) -> Result<Tx<'tx>> {
        return self.tx.upgrade().ok_or(Error::TxClosed);
    }

    // Root returns the root of the bucket.
    pub(crate) fn root(&self) -> PgId {
        return self.bucket.root_page();
    }

    /// Returns whether the bucket is writable.
    pub(crate) fn writeable(&self) -> bool {
        self.tx().unwrap().writable()
    }

    pub(crate) fn page_node(&self, root_page: PgId) -> (&Page, &Node) {
        todo!()
    }

    pub(crate) fn root_page(&self) -> PgId {
        return self.bucket.root_page();
    }
}

pub(crate) trait RawBucketApi<'tx> {
    // forEachPage iterates over every page in a bucket, including inline pages.
    fn for_each_page<F>(self, f: F) -> Result<()>
    where
        F: FnMut(&Page) -> Result<()>;

    // forEachPageNode iterates over every page (or node) in a bucket.
    // This also includes inline pages.
    fn for_each_page_node<F>(self, f: F) -> Result<()>
    where
        F: FnMut(&Node) -> Result<()>;

    // forEachPageNode iterates over every page (or node) in a bucket.
    // This also includes inline pages.
    fn _for_each_page_node<F>(self, root: PgId, depth: usize, f: F) -> Result<()>
    where
        F: FnMut(&Node) -> Result<()>;

    // spill writes all the nodes for this bucket to dirty pages.
    fn spill(self) -> Result<()>;

    // inlineable returns true if a bucket is small enough to be written inline
    // and if it contains no subbuckets. Otherwise, returns false.
    fn inlineable(&self) -> bool;

    // Returns the maximum total size of a bucket to make it a candidate for inlining.
    fn max_inline_bucket_size(&self) -> usize;

    // write allocates and writes a bucket to a byte slice.
    fn write(&mut self, p: &mut [u8]) -> &Bytes;

    // rebalance attempts to balance all nodes.
    fn rebalance(&mut self) -> Result<()>;

    // node creates a node from a page and associates it with a given parent.
    fn node(&self, pgid: PgId, parent: crate::node::WeakNode) -> Node;

    // free recursively frees all pages in the bucket.
    fn free(self) -> Result<()>;

    // pageNode returns the in-memory node, if it exists.
    // Otherwise, returns the underlying page.
    fn page_node(&self) -> Option<Node>;
}

// BucketStats records statistics about resources used by a bucket.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct BucketStats {
    // Page count statistics.
    // #[serde(rename = "branchPageN")]
    pub branch_page_n: i32, // number of logical branch pages
    // #[serde(rename = "branchOverflowN")]
    pub branch_overflow_n: i32, // number of physical branch overflow pages
    // #[serde(rename = "leafPageN")]
    pub leaf_page_n: i32, // number of logical leaf pages
    // #[serde(rename = "leafOverflowN")]
    pub leaf_overflow_n: i32, // number of physical leaf overflow pages

    // Tree statistics.
    // #[serde(rename = "keyN")]
    pub key_n: i32, // number of keys/value pairs
    pub depth: i32, // number of levels in B+tree

    // Page size utilization.
    // #[serde(rename = "branchAlloc")]
    pub branch_alloc: i32, // bytes allocated for physical branch pages
    // #[serde(rename = "branchInuse")]
    pub branch_inuse: i32, // bytes actually used for branch data
    // #[serde(rename = "leafAlloc")]
    pub leaf_alloc: i32, // bytes allocated for physical leaf pages
    // #[serde(rename = "leafInuse")]
    pub leaf_inuse: i32, // bytes actually used for leaf data

    // Bucket statistics
    // #[serde(rename = "bucketN")]
    pub bucket_n: i32, // total number of buckets including the top bucket
    // #[serde(rename = "inlineBucketN")]
    pub inline_bucket_n: i32, // total number on inlined buckets
    // #[serde(rename = "inlineBucketInuse")]
    pub inline_bucket_inuse: i32, // bytes used for inlined buckets (also accounted for in LeafInuse)
}

impl BucketStats {
    /// add adds the statistics from another BucketStats to this BucketStats.
    pub fn add(&mut self, other: BucketStats) {
        self.branch_page_n += other.branch_page_n;
        self.branch_overflow_n += other.branch_overflow_n;
        self.leaf_page_n += other.leaf_page_n;
        self.leaf_overflow_n += other.leaf_overflow_n;
        self.key_n += other.key_n;
        if self.depth < other.depth {
            self.depth = other.depth;
        }
        self.branch_alloc += other.branch_alloc;
        self.branch_inuse += other.branch_inuse;
        self.leaf_alloc += other.leaf_alloc;
        self.leaf_inuse += other.leaf_inuse;

        self.bucket_n += other.bucket_n;
        self.inline_bucket_n += other.inline_bucket_n;
        self.inline_bucket_inuse += other.inline_bucket_inuse;
    }
}

/// Implement the Add trait for BucketStats.
impl AddAssign for BucketStats {
    /// Adds the statistics from another BucketStats to this BucketStats.
    fn add_assign(&mut self, other: Self) {
        self.branch_page_n += other.branch_page_n;
        self.branch_overflow_n += other.branch_overflow_n;
        self.leaf_page_n += other.leaf_page_n;
        self.leaf_overflow_n += other.leaf_overflow_n;
        self.key_n += other.key_n;
        if self.depth < other.depth {
            self.depth = other.depth;
        }
        self.branch_alloc += other.branch_alloc;
        self.branch_inuse += other.branch_inuse;
        self.leaf_alloc += other.leaf_alloc;
        self.leaf_inuse += other.leaf_inuse;

        self.bucket_n += other.bucket_n;
        self.inline_bucket_n += other.inline_bucket_n;
        self.inline_bucket_inuse += other.inline_bucket_inuse;
    }
}

/// cloneBytes returns a copy of a given slice.
pub fn clone_bytes(v: &[u8]) -> Vec<u8> {
    v.to_vec()
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct BucketStructure {
    // #[serde(rename = "name")]
    pub name: String, // name of the bucket
    // #[serde(rename = "keyN")]
    pub key_n: i32, // number of key/value pairs
    // #[serde(rename = "buckets", skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<BucketStructure>, // child buckets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_bucket() {
        let mut in_bucket = InBucket::new(1, 1);

        in_bucket.inc_sequence();

        assert_eq!(in_bucket.root_page(), 1);
        assert_eq!(in_bucket.in_sequence(), 2);
    }

    #[test]
    fn test_bucket() {
        // let bucket = Bucket::new(10);
        // assert_eq!(bucket.bucket_n, 10);
        assert_eq!(MAX_KEY_SIZE, 32768);
    }

    #[test]
    fn test_bucket_stats() {
        // test default values
        let mut bucket_stats = BucketStats::default();
        bucket_stats.bucket_n = 5;
        bucket_stats.key_n = 2;

        // test custom values
        let other_bucket_stats = BucketStats {
            key_n: 2,
            bucket_n: 5,
            // other fields are default
            ..Default::default()
        };

        assert_eq!(bucket_stats.bucket_n, 5);
        assert_eq!(bucket_stats.key_n, 2);
        // add other bucket stats to bucket stats
        bucket_stats.add(other_bucket_stats);

        assert_eq!(bucket_stats.bucket_n, 10); // sum of bucket_n from both buckets
        assert_eq!(bucket_stats.key_n, 4); // sum of key_n from both buckets

        let mut bucket_stats = BucketStats::default();
        bucket_stats.bucket_n = 5;
        bucket_stats.key_n = 2;

        let mut other_bucket_stats = BucketStats::default();
        other_bucket_stats.bucket_n = 3;
        other_bucket_stats.key_n = 4;

        // add other bucket stats to bucket stats

        bucket_stats += (other_bucket_stats);

        assert_eq!(bucket_stats.bucket_n, 8); // sum of bucket_n from both buckets
        assert_eq!(bucket_stats.key_n, 6); // sum of key_n from both buckets
    }

    #[test]
    fn test_bucket_structure() {
        let bucket_structure = BucketStructure {
            name: String::from("example"),
            key_n: 10,
            children: vec![], // empty vector of child buckets
        };

        println!("{:?}", bucket_structure);

        assert_eq!(bucket_structure.name, "example");
        assert_eq!(bucket_structure.key_n, 10);
        assert!(bucket_structure.children.is_empty());
    }
}
