use crate::common::bucket::InBucket;
use crate::common::types::PgId;
use crate::common::types::{MAGIC, TxId, VERSION};
use crate::errors::Error;
use crate::errors::Result;
use fnv::FnvHasher;
use std::hash::Hasher;
use std::slice;
use std::{fmt, mem};

use super::page::{Page, PageFlags};
use super::types::PGID_NO_FREELIST;

///Meta Page Size
pub(crate) const META_PAGE_SIZE: usize = mem::size_of::<Meta>();

// 定义 Meta 结构体
#[derive(Debug, Default, Clone)]
#[repr(C)] // 确保 C 兼容的内存布局
pub(crate) struct Meta {
    /// database mime header
    magic: u32,
    /// database version
    version: u32,
    /// defined page size.
    /// u32 to be platform independent
    page_size: u32,
    /// haven't seen it's usage
    flags: u32,
    /// bucket that has root property changed
    /// during commits and transactions
    root: InBucket,
    /// free list page id
    freelist: PgId,
    /// pg_id high watermark
    pgid: PgId,
    /// transaction id
    txid: TxId,
    /// meta check_sum
    checksum: u64,
}

impl Meta {
    // Validate checks the marker bytes and version of the meta page to ensure it matches this binary.
    pub(crate) fn validate(&self) -> Result<()> {
        if self.magic != MAGIC {
            return Err(Error::Invalid);
        } else if self.version != VERSION as u32 {
            return Err(Error::VersionMismatch);
        } else if self.checksum != 0 && self.checksum != self.sum64() {
            return Err(Error::Checksum);
        }
        Ok(())
    }

    // Write writes the meta onto a page.
    pub(crate) fn write(&mut self, p: &mut Page) -> Result<()> {
        if self.root.root_page() >= self.pgid {
            panic!(
                "root bucket pgid ({}) above high water mark ({})",
                self.root.root_page(),
                self.pgid
            );
        } else if self.freelist >= self.pgid && self.freelist != PGID_NO_FREELIST {
            // TODO: reject pgidNoFreeList if !NoFreelistSync
            panic!(
                "freelist pgid ({}) above high water mark ({})",
                self.freelist, self.pgid
            );
        }

        // Page id is either going to be 0 or 1 which we can determine by the transaction ID.
        p.set_id(self.txid % 2);
        p.set_flags(PageFlags::META_PAGE);

        // Calculate the checksum.
        self.checksum = self.sum64();

        // Copy data to page's meta section
        self.copy(p.meta_mut());

        Ok(())
    }

    // Sum64 generates the checksum for the meta.
    pub fn sum64(&self) -> u64 {
        let mut h = FnvHasher::default();
        h.write(self.as_slice_no_checksum());
        h.finish()
    }

    //as slice bytes
    #[inline]
    pub(crate) fn as_slice(&self) -> &[u8] {
        let ptr = self as *const Meta as *const u8;
        unsafe { slice::from_raw_parts(ptr, self.byte_size()) }
    }

    #[inline]
    pub(crate) fn as_slice_no_checksum(&self) -> &[u8] {
        let ptr = self as *const Meta as *const u8;
        unsafe { slice::from_raw_parts(ptr, memoffset::offset_of!(Meta, checksum)) }
    }

    ///
    ///Meta size
    ///
    pub(crate) fn byte_size(&self) -> usize {
        META_PAGE_SIZE
    }

    // Getter 方法
    pub(crate) fn magic(&self) -> u32 {
        self.magic
    }

    pub(crate) fn version(&self) -> u32 {
        self.version
    }

    pub(crate) fn page_size(&self) -> u32 {
        self.page_size
    }

    pub(crate) fn flags(&self) -> u32 {
        self.flags
    }

    pub(crate) fn root_bucket(&self) -> &InBucket {
        &self.root
    }

    pub(crate) fn freelist(&self) -> PgId {
        self.freelist
    }

    pub(crate) fn pgid(&self) -> PgId {
        self.pgid
    }

    pub(crate) fn txid(&self) -> TxId {
        self.txid
    }

    pub(crate) fn checksum(&self) -> u64 {
        self.checksum
    }

    // Setter 方法
    pub(crate) fn set_magic(&mut self, v: u32) {
        self.magic = v;
    }

    pub(crate) fn set_version(&mut self, v: u32) {
        self.version = v;
    }

    pub(crate) fn set_page_size(&mut self, v: u32) {
        self.page_size = v;
    }

    pub(crate) fn set_flags(&mut self, v: u32) {
        self.flags = v;
    }

    pub(crate) fn set_root_bucket(&mut self, b: InBucket) {
        self.root = b;
    }

    pub(crate) fn set_freelist(&mut self, v: PgId) {
        self.freelist = v;
    }

    pub(crate) fn set_pgid(&mut self, id: PgId) {
        self.pgid = id;
    }

    pub(crate) fn set_txid(&mut self, id: TxId) {
        self.txid = id;
    }

    pub(crate) fn inc_txid(&mut self) {
        self.txid += 1;
    }

    pub(crate) fn dec_txid(&mut self) {
        self.txid -= 1;
    }

    pub(crate) fn set_checksum(&mut self, v: u64) {
        self.checksum = v;
    }

    // Copy copies one meta object to another.
    pub(crate) fn copy(&self, dest: &mut Meta) {
        //clone 可能有性能影响
        *dest = self.clone();
    }

    pub(crate) fn is_freelist_persisted(&self) -> bool {
        self.freelist != PGID_NO_FREELIST
    }
}

/// 实现 Meta 的格式化输出
impl fmt::Display for Meta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Meta {{   
                Version:  \t{}\n   
                Page Size:\t{} bytes\n   
                Flags:  \t\t0x{:08x}\n    
                Root:     \t<pgid={}>\n    
                Freelist: \t<pgid={}>\n    
                HWM:      \t<pgid={}>\n    
                Txn ID:  \t{}\n    
                Checksum: \t0x{:016x}\n
            }}",
            self.version,
            self.page_size,
            self.flags,
            self.root.root_page(),
            self.freelist,
            self.pgid,
            self.txid,
            self.checksum
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::common::types::DEFAULT_PAGE_SIZE;

    use super::*;

    #[test]
    fn test_meta() {
        let mut buf = vec![0u8; 1024];
        let mut page = Page::from_slice_mut(&mut buf);

        let mut meta = Meta {
            magic: MAGIC,
            version: VERSION,
            page_size: *DEFAULT_PAGE_SIZE as u32,
            flags: 0,
            root: Default::default(),
            freelist: 5,
            pgid: 10,
            txid: 2,
            checksum: 23,
        };

        let _ = meta.write(page);

        assert!(meta.validate().is_ok());
        assert_eq!(10, meta.pgid);
        assert!(page.is_meta_page());
        assert!(page.meta().pgid() == 10);
    }
}
