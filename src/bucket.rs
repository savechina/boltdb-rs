use std::cell::RefCell;
use std::collections::HashMap;
use std::mem;

use crate::common::bucket::InBucket;
use crate::common::inode::Key;
use crate::common::page::{OwnedPage, Page, PgId};
use crate::node::Node;
use crate::tx::{self, Tx, WeakTx};
// MaxKeySize is the maximum length of a key, in bytes.
const MAX_KEY_SIZE: usize = 32768;

// MaxValueSize is the maximum length of a value, in bytes.
const MAX_VALUE_SIZE: usize = (1 << 31) - 2;

const BUCKET_HEADER_SIZE: usize = mem::size_of::<Bucket>();

const MIN_FILL_PERCENT: f64 = 0.1;
const MAX_FILL_PERCENT: f64 = 1.0;

/// DefaultFillPercent is the percentage that split pages are filled.
/// This value can be changed by setting Bucket.FillPercent.
const DEFAULT_FILL_PERCENT: f64 = 0.5;

// Bucket represents a collection of key/value pairs inside the database.

#[derive(Debug)]
pub struct Bucket {
    pub(crate) local_bucket: InBucket,
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
}

