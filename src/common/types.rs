//!
//!  Bolt default type declare
//!
use crate::common::page::PgId;
use once_cell::sync::Lazy;
use page_size;
use std::time::Duration;

// 最大 mmap 步长为 1GB
// MaxMmapStep is the largest step that can be taken when remapping the mmap.
pub(crate) const MAX_MMAP_STEP: usize = 1 << 30; // 1GB

// 数据文件格式版本
// Version represents the data file format version.
pub(crate) const VERSION: u32 = 2;

// Bolt DB 文件标识符
// Magic represents a marker value to indicate that a file is a Bolt DB.
pub(crate) const MAGIC: u32 = 0xED0CDAED;

// 表示没有空闲列表的页面组 ID
pub(crate) const PGID_NO_FREELIST: PgId = 0xFFFFFFFFFFFFFFFF;

// 页面最大分配大小
// DO NOT EDIT. Copied from the "bolt" package.
pub(crate) const PAGE_MAX_ALLOC_SIZE: usize = 0xFFFFFFF;

// 是否忽略 NoSync 字段
// IgnoreNoSync specifies whether the NoSync field of a DB is ignored when
// syncing changes to a file. This is required as some operating systems,
// such as OpenBSD, do not have a unified buffer cache (UBC) and writes
// must be synchronized using the msync(2) syscall.
pub(crate) const IGNORE_NO_SYNC: bool = cfg!(target_os = "openbsd");

// 默认值
// Default values if not set in a DB instance.
pub(crate) const DEFAULT_MAX_BATCH_SIZE: usize = 1000;

pub(crate) const DEFAULT_MAX_BATCH_DELAY: Duration = Duration::from_millis(10);

pub(crate) const DEFAULT_ALLOC_SIZE: usize = 16 * 1024 * 1024;

// 默认页面大小
// DefaultPageSize is the default page size for db which is set to the OS page size.
pub(crate) static DEFAULT_PAGE_SIZE: Lazy<usize> = Lazy::new(|| page_size::get());

// 内部事务标识符
// Txid represents the internal transaction identifier.
pub(crate) type TxId = u64;

//Byte 字节类型
pub type Byte = u8;

///
/// 单元测试
/// #[cfg(test)]
///
#[cfg(test)]
mod tests {
    // 注意这个惯用法：在 tests 模块中，从外部作用域导入所有名字。
    use super::*;

    #[test]
    fn test_page_size() {
        let page_size = *DEFAULT_PAGE_SIZE;

        println!("system page size:{}", page_size);
        assert_eq!(16384, page_size);
    }
}
