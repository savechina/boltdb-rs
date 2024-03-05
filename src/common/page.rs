//!Page
//!

use super::bucket::InBucket;
use super::meta::{Meta, META_PAGE_SIZE};
use super::{load_bucket, must_align};
use std::borrow::{Borrow, BorrowMut};
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut, RangeBounds};
use std::slice::{self, Iter};

use bitflags::bitflags;
use std::ptr;

//Page Id
pub(crate) type PgId = u64;

/// Page header size
pub(crate) const PAGE_HEADER_SIZE: usize = mem::size_of::<Page>();

const MIN_KEYS_PER_PAGE: i32 = 2;

/// BranchPageElement size
const BRANCH_PAGE_ELEMENT_SIZE: usize = mem::size_of::<BranchPageElement>();

/// LeafPageElement size
const LEAF_PAGE_ELEMENT_SIZE: usize = mem::size_of::<LeafPageElement>();

/// PgId size
pub(crate) const PGID_SIZE: usize = mem::size_of::<PgId>();

bitflags! {
    // 定义 PageFlags bit 标识
    #[derive(Debug,PartialEq, Eq,Clone, Copy)]
    pub struct PageFlags :u16 {
        /// Either branch or bucket page
        const BRANCH_PAGE = 0x01;
        // Leaf Page
        const LEAF_PAGE = 0x02;
        //Meta Page
        const META_PAGE  = 0x04;
        //Freelist Page
        const FREELIST_PAGE = 0x10;
    }

}

impl Display for PageFlags {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:0x}", self)
    }
}

// u16
pub(crate) const BUCKET_LEAF_FLAG: u32 = 0x01;

///////////////////////////////////////////////////////////
//    Page 结构体基础对象
///////////////////////////////////////////////////////////

/// Page 结构体
///
/// Page Header:
///   |PgId(u64)|flags(u16)|count(u16)|over_flow
///
/// Page Size = count + over_flow*sizeof(Page)
#[derive(Debug)]
#[repr(C)]
pub(crate) struct Page {
    id: PgId,
    flags: PageFlags,
    count: u16,
    overflow: u32,
    // PhantomData not occupy real memory
    ptr: PhantomData<u8>,
}

/// 实现默认构造函数值
impl Default for Page {
    fn default() -> Self {
        Self {
            id: 0,
            flags: PageFlags::BRANCH_PAGE,
            count: 0,
            overflow: 0,
            ptr: PhantomData::default(),
        }
    }
}

impl Page {
    /// 实现 Page 的构造函数
    pub(crate) fn new(id: PgId, flags: PageFlags, count: u16, overflow: u32) -> Self {
        Self {
            id,
            flags,
            count,
            overflow,
            ptr: PhantomData::default(),
        }
    }

    ///page type
    pub(crate) fn typ(&self) -> String {
        if self.is_branch_page() {
            return String::from("branch");
        } else if self.is_leaf_page() {
            return String::from("leaf");
        } else if self.is_meta_page() {
            return String::from("meta");
        } else if self.is_freelist_page() {
            return String::from("freelist");
        }

        return format!("unknown<{:0x}>", self.flags);
    }

    pub(crate) fn is_branch_page(&self) -> bool {
        // self.flags.contains(PageFlags::BRANCH_PAGE);
        matches!(self.flags, PageFlags::BRANCH_PAGE)
    }

    pub(crate) fn is_leaf_page(&self) -> bool {
        self.flags.contains(PageFlags::LEAF_PAGE)
    }

    pub(crate) fn is_meta_page(&self) -> bool {
        self.flags.contains(PageFlags::META_PAGE)
    }

    pub(crate) fn is_freelist_page(&self) -> bool {
        self.flags.contains(PageFlags::FREELIST_PAGE)
    }

    // Meta returns a pointer to the metadata section of the page.
    pub fn meta(&self) -> &Meta {
        // 使用 unsafe 块来执行不安全的内存操作。
        unsafe {
            // 使用 `mem::transmute` 函数将指针转换为 `&Meta` 类型。

            let offset = self.get_data_ptr();

            return mem::transmute(offset as *const Meta);
        }

        /*
        // 使用 unsafe 块来执行不安全的内存操作。
         unsafe {
             // 使用将指针移到到Page 的数据部分的开始位置。
             let meta_ptr = self.get_data_ptr() as *const Meta;

             // 将元数据指针转换为 `&Meta` 类型。
             &*(meta_ptr as *const Meta)
         }
         */
    }

    pub(crate) fn meta_mut(&self) -> &mut Meta {
        unsafe {
            let data_ptr = self.get_data_ptr();

            return &mut *(data_ptr as *mut Meta);
        }
    }

    pub(crate) fn fast_check(&self, id: PgId) {
        //check pgid
        assert!(
            self.id == id,
            "Page expected to be: {}, but self identifies as {}",
            id,
            self.id
        );

        // check if only one flag is set
        let has_multiple_flags = self.is_meta_page()
            || self.is_branch_page()
            || self.is_leaf_page()
            || self.is_freelist_page();

        assert!(
            has_multiple_flags,
            "page {}: has unexpected type/flags: {:x}",
            self.id, self.flags
        );
    }

    pub(crate) fn leaf_page_element(&self, index: usize) -> &LeafPageElement {
        &self.leaf_page_elements()[index]
    }

    pub(crate) fn leaf_page_element_mut(&mut self, index: usize) -> &mut LeafPageElement {
        &mut self.leaf_page_elements_mut()[index]
    }

    pub(crate) fn leaf_page_elements(&self) -> &[LeafPageElement] {
        unsafe {
            if self.count == 0 {
                return &[]; // Return an empty slice
            }

            let data_ptr = self.get_data_ptr();

            // Create a slice from the raw data, treating it as an array of leafPageElements
            slice::from_raw_parts(data_ptr as *const LeafPageElement, self.count as usize)
        }
    }

    pub(crate) fn leaf_page_elements_mut(&mut self) -> &mut [LeafPageElement] {
        unsafe {
            if self.count == 0 {
                return &mut []; // Return an empty slice
            }

            let data_ptr = self.get_data_ptr();

            // Create a slice from the raw data, treating it as an array of leafPageElements
            slice::from_raw_parts_mut(data_ptr as *mut LeafPageElement, self.count as usize)
        }
    }

    pub(crate) fn branch_page_element(&self, index: usize) -> &BranchPageElement {
        &self.branch_page_elements()[index]
    }

    pub(crate) fn branch_page_element_mut(&mut self, index: usize) -> &mut BranchPageElement {
        &mut self.branch_page_elements_mut()[index]
    }

    pub(crate) fn branch_page_elements(&self) -> &[BranchPageElement] {
        unsafe {
            if self.count == 0 {
                return &[]; // Return an empty slice
            }

            let data_ptr = self.get_data_ptr();

            // Create a slice from the raw data, treating it as an array of leafPageElements
            slice::from_raw_parts(data_ptr as *const BranchPageElement, self.count as usize)
        }
    }

    pub(crate) fn branch_page_elements_mut(&self) -> &mut [BranchPageElement] {
        unsafe {
            if self.count == 0 {
                return &mut []; // Return an empty slice
            }

            let data_ptr = self.get_data_ptr();

            // Create a slice from the raw data, treating it as an array of leafPageElements
            slice::from_raw_parts_mut(data_ptr as *mut BranchPageElement, self.count as usize)
        }
    }

    // Returns a slice to the free list section of the page.
    pub(crate) fn free_list(&self) -> &[PgId] {
        assert!(
            self.is_freelist_page(),
            "can't get freelist page IDs from a non-freelist page:{:02x}",
            self.flags
        );

        unsafe { slice::from_raw_parts(self.get_data_ptr() as *const PgId, self.count as usize) }
    }

    // Returns a mut slice to the free list section of the page.
    pub(crate) fn free_list_mut(&mut self) -> &mut [PgId] {
        assert!(
            self.is_freelist_page(),
            "can't get freelist page IDs from a non-freelist page:{:02x}",
            self.flags
        );

        unsafe {
            std::slice::from_raw_parts_mut(
                self.get_data_mut_ptr() as *mut PgId,
                self.count as usize,
            )
        }
    }

    pub fn freelist_page_count(&self) -> (usize, usize) {
        assert!(
            self.is_freelist_page(),
            "can't get freelist page count from a non-freelist page: {:02x}",
            self.flags
        );

        // If the page.count is at the max uint16 value (64k) then it's considered
        // an overflow and the size of the freelist is stored as the first element.
        let count = self.count as usize;

        if count == 0xFFFF {
            let data_ptr = self.get_data_ptr() as *const PgId;
            let count = (data_ptr) as usize; // Get count from first element

            if count >= std::usize::MAX {
                panic!("leading element count overflows usize");
            }
            return (1, count);
        }

        (0, count)
    }

    pub fn freelist_page_ids(&self) -> &[PgId] {
        assert!(
            self.is_freelist_page(),
            "can't get freelist page IDs from a non-freelist page: {:02x}",
            self.flags
        );

        let (idx, count) = self.freelist_page_count();

        if count == 0 {
            return &[];
        }

        unsafe {
            let data_ptr = self.get_data_ptr();

            std::slice::from_raw_parts(data_ptr as *const PgId, count)
        }
    }

    pub(crate) fn page_element_size(&self) -> usize {
        if self.is_leaf_page() {
            return LEAF_PAGE_ELEMENT_SIZE;
        }
        return BRANCH_PAGE_ELEMENT_SIZE;
    }

    pub fn id(&self) -> PgId {
        self.id
    }

    pub fn set_id(&mut self, target: PgId) {
        self.id = target;
    }

    pub fn flags(&self) -> PageFlags {
        self.flags
    }

    pub fn set_flags(&mut self, flags: PageFlags) {
        self.flags = flags;
    }

    pub fn count(&self) -> u16 {
        self.count
    }

    pub fn set_count(&mut self, count: u16) {
        self.count = count;
    }

    pub fn overflow(&self) -> u32 {
        self.overflow
    }

    pub fn set_overflow(&mut self, overflow: u32) {
        self.overflow = overflow;
    }

    pub fn to_string(&self) -> String {
        format!(
            "ID: {}, Type: {}, count: {}, overflow: {}",
            self.id,
            self.typ(),
            self.count,
            self.overflow
        )
    }

    pub(crate) fn pgid(&self, index: usize) -> &PgId {
        &self.pg_ids()[index]
    }

    pub(crate) fn pg_ids(&self) -> &[PgId] {
        unsafe { slice::from_raw_parts(self.get_data_ptr() as *const PgId, self.count as usize) }
    }

    pub(crate) fn pg_ids_mut(&mut self) -> &mut [PgId] {
        unsafe {
            std::slice::from_raw_parts_mut(
                self.get_data_mut_ptr() as *mut PgId,
                self.count as usize,
            )
        }
    }

    #[inline]
    pub(crate) fn get_data_mut_ptr(&mut self) -> *mut u8 {
        unsafe { (&mut self.ptr as *mut PhantomData<u8> as *mut u8) }
    }

    #[inline]
    pub(crate) fn get_data_ptr(&self) -> *const u8 {
        unsafe { (&self.ptr as *const PhantomData<u8> as *const u8) }
    }

    #[inline]
    pub(crate) fn get_data_slice(&self) -> &[u8] {
        let ptr = self.get_data_ptr();
        unsafe { slice::from_raw_parts(ptr, self.byte_size() - PAGE_HEADER_SIZE) }
    }

    #[inline]
    pub(crate) fn as_slice(&self) -> &[u8] {
        let ptr: *const u8 = self as *const Page as *const u8;
        unsafe { slice::from_raw_parts(ptr, self.byte_size()) }
    }

    #[inline]
    pub(crate) fn as_slice_mut(&mut self) -> &mut [u8] {
        let ptr = self as *mut Page as *mut u8;
        unsafe { slice::from_raw_parts_mut(ptr, self.byte_size()) }
    }

    #[inline]
    pub(crate) fn from_slice(buffer: &[u8]) -> &Page {
        unsafe { &*(buffer.as_ptr() as *const Page) }
    }

    #[inline]
    pub(crate) fn from_slice_mut(mut buffer: &mut [u8]) -> &mut Self {
        unsafe { &mut *(buffer.as_mut_ptr() as *mut Page) }
    }

    pub(crate) fn byte_size(&self) -> usize {
        let mut size = PAGE_HEADER_SIZE;

        match self.flags {
            PageFlags::BRANCH_PAGE => {
                let branch = self.branch_page_elements();
                let len = branch.len();
                if len > 0 {
                    let last_branch = branch.last().unwrap();
                    size += (len - 1) * BRANCH_PAGE_ELEMENT_SIZE;
                    size += (last_branch.pos() + last_branch.ksize()) as usize;
                }
            }
            PageFlags::LEAF_PAGE => {
                let leaves = self.leaf_page_elements();
                let len = leaves.len();
                if len > 0 {
                    let last_leaf = leaves.last().unwrap();
                    size += (len - 1) * LEAF_PAGE_ELEMENT_SIZE;
                    size += (last_leaf.pos + last_leaf.ksize + last_leaf.vsize) as usize;
                }
            }
            PageFlags::META_PAGE => {
                size += META_PAGE_SIZE;
            }
            PageFlags::FREELIST_PAGE => {
                size += self.pg_ids().len() * mem::size_of::<PgId>();
            }
            _ => panic!("Unknown page flag: {}", self.flags),
        }
        size
    }
}

impl fmt::Display for Page {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{ ID: {}, Type: {}, count: {}, overflow: {} }}",
            self.id,
            self.typ(),
            self.count,
            self.overflow
        )
    }
}

impl ToOwned for Page {
    type Owned = OwnedPage;

    fn to_owned(&self) -> Self::Owned {
        let ptr = self as *const Page as *const u8;
        unsafe {
            let slice = slice::from_raw_parts(ptr, self.byte_size()).to_owned();
            OwnedPage::from_vec(slice)
        }
    }
}

///////////////////////////////////////////////////////////
//              Page 页面元素 BranchPageElement
///////////////////////////////////////////////////////////
/**
 * BranchPageElement
 */
#[derive(Debug, Default)]
#[repr(C)]
pub(crate) struct BranchPageElement {
    pos: u32,
    ksize: u32,
    pgid: PgId,
}

impl BranchPageElement {
    pub(crate) fn pos(&self) -> u32 {
        self.pos
    }

    pub(crate) fn set_pos(&mut self, pos: u32) {
        self.pos = pos;
    }

    pub fn ksize(&self) -> u32 {
        self.ksize
    }

    pub fn set_ksize(&mut self, size: u32) {
        self.ksize = size;
    }

    pub fn pgid(&self) -> PgId {
        self.pgid
    }

    pub fn set_pgid(&mut self, v: PgId) {
        self.pgid = v;
    }

    /// Key returns a byte slice of the node key.
    pub(crate) fn key(&self) -> &[u8] {
        must_align(self);

        unsafe {
            let key_ptr = ptr::addr_of!(self.pos) as *const u8;
            std::slice::from_raw_parts(key_ptr, self.ksize as usize)
        }
    }

    #[inline]
    pub(crate) const fn as_ptr(&self) -> *const u8 {
        self as *const Self as *const u8
    }
}

///////////////////////////////////////////////////////////
//              Page 页面元素 LeafPageElement
///////////////////////////////////////////////////////////

///
/// LeafPageElement represents a node on a leaf page.
///
#[derive(Debug, Default)]
#[repr(C)]
pub(crate) struct LeafPageElement {
    flags: u32,
    pub(crate) pos: u32,
    pub(crate) ksize: u32,
    pub(crate) vsize: u32,
}

impl LeafPageElement {
    pub fn new(flags: u32, pos: u32, ksize: u32, vsize: u32) -> Self {
        Self {
            flags,
            pos,
            ksize,
            vsize,
        }
    }

    // Getters and setters for flags, pos, ksize, vsize (similar to BranchPageElement)

    pub(crate) fn set_ksize(&mut self, len: u32) {
        self.ksize = len;
    }

    pub(crate) fn set_vsize(&mut self, len: u32) {
        self.vsize = len;
    }

    pub(crate) fn flags(&self) -> u32 {
        self.flags
    }

    pub(crate) fn set_flags(&mut self, flags: u32) {
        self.flags = flags;
    }

    pub(crate) fn pos(&self) -> u32 {
        self.pos
    }

    pub(crate) fn set_pos(&mut self, pos: u32) {
        self.pos = pos;
    }

    /// Key returns a byte slice of the node key.
    pub fn key(&self) -> &[u8] {
        unsafe {
            let key_ptr = ptr::addr_of!(self.pos) as *const u8;
            std::slice::from_raw_parts(key_ptr, self.ksize as usize)
        }
    }

    /// Value returns a byte slice of the node value.
    pub(crate) fn value(&self) -> &[u8] {
        must_align(self);

        unsafe {
            let value_ptr = ptr::addr_of!(self.vsize) as *const u8; // Adjust pointer offset

            slice::from_raw_parts(value_ptr, self.vsize as usize)
        }
    }

    pub(crate) fn is_bucket_entry(&self) -> bool {
        (self.flags & BUCKET_LEAF_FLAG) != 0
    }

    pub(crate) fn bucket(&self) -> Option<&InBucket> {
        if self.is_bucket_entry() {
            // Assuming LoadBucket loads a bucket from a byte slice
            unsafe { load_bucket(self.value()) }
        } else {
            None
        }
    }

    #[inline]
    pub(crate) const fn as_ptr(&self) -> *const u8 {
        self as *const Self as *const u8
    }
}

///
///
#[derive(Clone, Debug, Default, PartialOrd, PartialEq)]
pub(crate) struct PgIds {
    pgids: Vec<PgId>,
}

impl From<Vec<PgId>> for PgIds {
    fn from(v: Vec<u64>) -> Self {
        PgIds { pgids: v }
    }
}

impl PgIds {
    #[inline]
    pub fn len(&self) -> usize {
        self.pgids.len()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, u64> {
        self.pgids.iter()
    }

    #[inline]
    pub fn sort(&mut self) {
        self.pgids.sort();
    }

    #[inline]
    pub fn as_slice(&self) -> &Vec<PgId> {
        &self.pgids
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pgids.is_empty()
    }

    #[inline]
    pub fn push(&mut self, pgid: PgId) {
        self.pgids.push(pgid);
    }

    #[inline]
    pub fn to_vec(self) -> Vec<PgId> {
        self.pgids
    }

    #[inline]
    pub fn as_ref_vec(&self) -> &Vec<PgId> {
        &self.pgids
    }

    #[inline]
    pub fn drain<R>(&mut self, range: R) -> Vec<u64>
    where
        R: RangeBounds<usize>,
    {
        self.pgids.drain(range).collect::<Vec<_>>()
    }

    /// Merge pgids copies the sorted union of a and b into dst.
    #[inline]
    pub fn extend_from_slice(&mut self, slice: Self) {
        //extend from anther slice pgids
        self.pgids.extend_from_slice(&*slice.pgids);

        //first sorted
        self.pgids.sort();

        //Removes consecutive repeated elements in the vector according to the
        self.pgids.dedup();

        //sorted
        // self.pgids.sort();
    }
}

// represents human-readable information about a page.
#[derive(Debug, Default)]
pub(crate) struct PageInfo {
    id: u64,
    typ: u16,
    count: usize,
    overflow_count: usize,
}

impl PageInfo {
    /// Creates a new [`PageInfo`].
    pub(crate) fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    ///Getter and Setter
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn typ(&self) -> u16 {
        self.typ
    }

    pub(crate) fn count(&self) -> usize {
        self.count
    }

    pub(crate) fn overflow_count(&self) -> usize {
        self.overflow_count
    }

    pub(crate) fn set_id(&mut self, id: u64) {
        self.id = id;
    }

    pub(crate) fn set_typ(&mut self, typ: u16) {
        self.typ = typ;
    }

    pub(crate) fn set_count(&mut self, count: usize) {
        self.count = count;
    }

    pub(crate) fn set_overflow_count(&mut self, overflow_count: usize) {
        self.overflow_count = overflow_count;
    }

    pub(crate) fn id_mut(&mut self) -> &mut u64 {
        &mut self.id
    }

    pub(crate) fn typ_mut(&mut self) -> &mut u16 {
        &mut self.typ
    }

    pub(crate) fn count_mut(&mut self) -> &mut usize {
        &mut self.count
    }

    pub(crate) fn overflow_count_mut(&mut self) -> &mut usize {
        &mut self.overflow_count
    }
}

///
///OwnedPage is  Page impl ToOwned  trait struct
///
#[derive(Clone)]
#[repr(align(64))]
pub(crate) struct OwnedPage {
    ///Page bytes buffer
    page: Vec<u8>,
}

impl OwnedPage {
    ///Create new [`OwnedPage`] instance ,and init size page buffer
    ///
    pub(crate) fn new(size: usize) -> Self {
        Self {
            page: vec![0u8; size],
        }
    }

    /// build OwnedPage from Vec<u8> buffer
    pub(crate) fn from_vec(buf: Vec<u8>) -> Self {
        Self { page: buf }
    }

    /// reserve capacity of underlying vector to size
    #[allow(dead_code)]
    pub(crate) fn reserve(&mut self, size: usize) {
        self.page.reserve(size);
    }

    /// Returns pointer to page structure
    #[inline]
    pub(crate) fn as_ptr(&self) -> *const u8 {
        self.page.as_ptr()
    }

    /// Returns pointer to page structure
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        self.page.as_mut_ptr()
    }

    /// Returns binary serialized buffer pf a page
    #[inline]
    pub(crate) fn buf(&self) -> &[u8] {
        &self.page
    }

    /// Returns binary serialized muttable buffer of a page
    #[inline]
    pub(crate) fn buf_mut(&mut self) -> &mut [u8] {
        &mut self.page
    }

    /// Returns page size
    #[inline]
    pub(crate) fn size(&self) -> usize {
        self.page.len()
    }
}

impl Borrow<Page> for OwnedPage {
    #[inline]
    fn borrow(&self) -> &Page {
        unsafe { &*(self.page.as_ptr() as *const Page) }
    }
}

impl BorrowMut<Page> for OwnedPage {
    #[inline]
    fn borrow_mut(&mut self) -> &mut Page {
        unsafe { &mut *(self.page.as_mut_ptr() as *mut Page) }
    }
}

impl Deref for OwnedPage {
    type Target = Page;

    #[inline]
    fn deref(&self) -> &Page {
        self.borrow()
    }
}

impl DerefMut for OwnedPage {
    #[inline]
    fn deref_mut(&mut self) -> &mut Page {
        self.borrow_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_type() {
        let page = Page {
            flags: PageFlags::LEAF_PAGE,
            ..Default::default()
        };

        let typ = page.typ();
        println!("page type:{}", typ);
        if typ != "leaf" {
            panic!("is not leaf page")
        }

        // Test branch page
        assert_eq!(
            Page {
                flags: PageFlags::BRANCH_PAGE,
                ..Default::default()
            }
            .typ(),
            "branch"
        );

        // Test leaf page
        assert_eq!(
            Page {
                flags: PageFlags::LEAF_PAGE,
                ..Default::default()
            }
            .typ(),
            "leaf"
        );

        // Test meta page
        assert_eq!(
            Page {
                flags: PageFlags::META_PAGE,
                ..Default::default()
            }
            .typ(),
            "meta"
        );

        // Test freelist page
        assert_eq!(
            Page {
                flags: PageFlags::FREELIST_PAGE,
                ..Default::default()
            }
            .typ(),
            "freelist"
        );

        // Test unknown flag
        // assert_eq!(
        //     Page {
        //         flags: PageFlags::from_bits(20000).unwrap(),
        //         ..Default::default()
        //     }
        //     .typ(),
        //     format!("unknown<{:x}>", 20000)
        // );
    }

    #[test]
    fn test_pgids_merge() {
        let mut pgids_a: PgIds = PgIds::from(vec![12323, 334, 3445, 4456, 333]);
        let pgids_b: PgIds = PgIds {
            pgids: vec![12323, 4567, 3445, 3489, 33356],
        };

        println!("pgids a is: {:?}", pgids_a);
        println!("pgids b is: {:?}", pgids_b);

        assert_eq!(pgids_a.len(), 5);

        pgids_a.extend_from_slice(pgids_b);

        println!("pgids a is: {:?}", pgids_a);

        assert_eq!(pgids_a.len(), 8);
    }

    #[test]
    fn test_page_buffer() {
        let page: Page = Page::default();

        println!("new page from default :{}", page);

        println!("page ptr:{:p}", &page);
        println!("page id:{:p}", &page.id);
        println!("page count:{:p}", &page.count);
        println!("page ptr pathomdata:{:p}", page.get_data_ptr());

        let mut page: Page = Page::default();
        page.set_id(2);
        page.set_flags(PageFlags::LEAF_PAGE);
        page.set_count(2);
        page.set_overflow(0);

        let buffer = page.as_slice();
        let mut new_page = Page::from_slice(buffer);

        assert_eq!(buffer, new_page.as_slice());
    }

    #[test]
    fn test_page_new() {
        let mut buf = vec![0u8; 1024];
        let mut page = Page::from_slice_mut(&mut buf);

        assert_eq!(page.id, 0);
        assert_eq!(page.count, 0);

        page.set_id(36);
        assert_eq!(page.id, 36);

        page.set_flags(PageFlags::META_PAGE);
        assert_eq!(page.flags, PageFlags::META_PAGE);

        let mut page: OwnedPage = OwnedPage::new(1024);
        page.set_id(26);
        page.set_count(36);

        assert_eq!(page.id(), 26);
    }

    #[test]
    fn test_read_ownedpage() {
        let mut buf: Vec<u8> = vec![0u8; 4096];
        let len: usize = 2;

        let mut page = Page::from_slice_mut(&mut buf);

        page.set_id(123);
        page.set_flags(PageFlags::LEAF_PAGE);
        page.set_count(len as u16);
        page.set_overflow(0); 

       
        let ptr = page.get_data_ptr();

        let nodes = unsafe { slice::from_raw_parts_mut(ptr as *mut LeafPageElement, len) };

        assert_eq!(nodes[0].as_ptr(), ptr);

        // 0 node
        nodes[0].set_pos(32);
        nodes[0].set_ksize(5);
        nodes[0].set_flags(1);
        nodes[0].set_vsize(5);

        // 1 node
        nodes[1] = LeafPageElement {
            flags: 0,
            pos: 26,
            ksize: 3,
            vsize: 4,
        }; 

        //to read leaf element
        let elem= page.leaf_page_element(0);

        assert_eq!(elem.pos,32);
        assert_eq!(elem.ksize,5);
        assert_eq!(elem.vsize,5);
        assert_eq!(elem.flags(),1);

        let elem1 =page.leaf_page_element(1);
        assert_eq!(elem1.pos,26);
        assert_eq!(elem1.ksize,3);
        assert_eq!(elem1.vsize,4);
        assert_eq!(elem1.flags(),0); 
 

    }

    #[test]
    fn test_write_ownedpage() {
        let mut buf: Vec<u8> = vec![0u8; 4096];
        let len: usize = 2;

        let mut page = Page::from_slice_mut(&mut buf);

        page.set_id(123);
        page.set_flags(PageFlags::LEAF_PAGE);
        page.set_count(len as u16);
        page.set_overflow(0);

        assert_eq!(page.typ(), "leaf");

        let ptr = page.get_data_ptr();

        let nodes = unsafe { slice::from_raw_parts_mut(ptr as *mut LeafPageElement, len) };

        assert_eq!(nodes[0].as_ptr(), ptr);
        nodes[0].set_pos(32);
        nodes[0].set_ksize(5);
        nodes[0].set_flags(1);
        nodes[0].set_vsize(5);

        nodes[1] = LeafPageElement {
            flags: 0,
            pos: 26,
            ksize: 3,
            vsize: 4,
        };

        assert_eq!(page.typ(), "leaf");

        println!(
            "page head:{}, size:{},leaf size:{}, buffer:{:?}",
            PAGE_HEADER_SIZE,
            page.byte_size(),
            (len * LEAF_PAGE_ELEMENT_SIZE + 7 + 10),
            page.as_slice(),
        );

        let ownedPage = page.to_owned();

        println!("owned: {}", ownedPage.page.len())
    }
}
