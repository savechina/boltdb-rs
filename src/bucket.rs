use std::mem;

use crate::common::bucket::InBucket;
use crate::common::page::Page;
use crate::tx::{self, Tx};
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
pub struct Bucket {
    bucket: InBucket,
    tx: Tx, // the associated transaction
    // buckets  :map[string]*Bucket ,// subbucket cache
    page: Page, // inline page reference
    // rootNode *node             , // materialized node for the root page.
    // nodes    map[pgid]*node   ,  // node cache

    // Sets the threshold for filling nodes when they split. By default,
    // the bucket will fill to 50% but it can be useful to increase this
    // amount if you know that your write workloads are mostly append-only.
    //
    // This is non-persisted across transactions so it must be set in every Tx.
    fill_percent: f64,
}
