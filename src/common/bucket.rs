//!bucket construct boltdb base construction,for organization data unit
//!
//!

use std::fmt;

use crate::common::page::Page;
use crate::common::types::PgId;

// 使用 std::mem::size_of 函数获取 InBucket 结构体的字节大小
const BUCKET_HEADER_SIZE: usize = std::mem::size_of::<InBucket>();

// InBucket represents the on-file representation of a bucket.
// This is stored as the "value" of a bucket key. If the bucket is small enough,
// then its root page can be stored inline in the "value", after the bucket
// header. In the case of inline buckets, the "root" will be 0.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)] // 确保 C 兼容的内存布局
pub(crate) struct InBucket {
    root: PgId,    // page id of the bucket's root-level page
    sequence: u64, // monotonically incrementing, used by NextSequence()
}

// 实现 InBucket 方法
impl InBucket {
    /// 实现 InBucket 的构造函数
    pub(crate) fn new(root: PgId, sequence: u64) -> Self {
        Self { root, sequence }
    }

    ///root_page return root Page Pgid
    pub(crate) fn root_page(&self) -> PgId {
        self.root
    }

    pub(crate) fn set_root_page(&mut self, id: PgId) {
        self.root = id;
    }

    /// in_sequence returns the sequence. The reason why not naming it `Sequence`
    /// is to avoid duplicated name as `(*Bucket) Sequence()`
    pub(crate) fn in_sequence(&self) -> u64 {
        self.sequence
    }

    /// set_in_sequence will to set new sequence
    pub(crate) fn set_in_sequence(&mut self, sequence: u64) {
        self.sequence = sequence;
    }
    ///inc_sequence return next sequence
    pub(crate) fn inc_sequence(&mut self) {
        self.sequence += 1;
    }

    // 使用 unsafe 代码进行指针转换
    pub(crate) unsafe fn inline_page(&self, v: &[u8]) -> &Page {
        &*(v.as_ptr().add(BUCKET_HEADER_SIZE) as *const Page)
    }
}

// 实现 InBucket 的格式化输出
impl fmt::Display for InBucket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<pgid={},seq={}>", self.root, self.sequence)
    }
}

/// 实现 From<InBucket> for String
impl From<InBucket> for String {
    fn from(bucket: InBucket) -> String {
        format!("<pgid={},seq={}>", bucket.root, bucket.sequence)
    }
}
