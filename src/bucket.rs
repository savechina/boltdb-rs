use crate::common::bucket::InBucket;
use crate::common::inode::Key;
use crate::common::page::{OwnedPage, Page, PgId};
use crate::errors::{Error, Result};
use crate::node::Node;
use crate::tx::{self, Tx, WeakTx};
// use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem;
use std::ops::AddAssign;

// MaxKeySize is the maximum length of a key, in bytes.
const MAX_KEY_SIZE: usize = 32768;

// MaxValueSize is the maximum length of a value, in bytes.
const MAX_VALUE_SIZE: usize = (1 << 31) - 2;

const BUCKET_HEADER_SIZE: usize = mem::size_of::<Bucket>();

pub(crate) const MIN_FILL_PERCENT: f64 = 0.1;
pub(crate) const MAX_FILL_PERCENT: f64 = 1.0;

/// DefaultFillPercent is the percentage that split pages are filled.
/// This value can be changed by setting Bucket.FillPercent.
pub(crate) const DEFAULT_FILL_PERCENT: f64 = 0.5;

// Bucket represents a collection of key/value pairs inside the database.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Bucket {
    pub(crate) bucket: InBucket,
    // the associated transaction, WeakTx
    pub(crate) tx: WeakTx,
    // subbucket cache
    pub(crate) buckets: RefCell<HashMap<Key, Bucket>>,
    // inline page reference
    pub(crate) page: Option<OwnedPage>,
    // materialized node for the root page
    pub(crate) root_node: Option<Node>,
    // node cache
    // TODO: maybe use refHashMap
    pub(crate) nodes: RefCell<HashMap<PgId, Node>>,
    // Sets the threshold for filling nodes when they split. By default,
    // the bucket will fill to 50% but it can be useful to increase this
    // amount if you know that your write workloads are mostly append-only.
    //
    // This is non-persisted across transactions so it must be set in every Tx.
    pub(crate) fill_percent: f64,
}

impl Bucket {
    pub(crate) fn node(&self, child_pgid: PgId, from: crate::node::WeakNode) -> Node {
        todo!()
    }

    // Tx returns the tx of the bucket.
    pub(crate) fn tx(&self) -> Result<Tx> {
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
