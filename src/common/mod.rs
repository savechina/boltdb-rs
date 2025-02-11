//!
//! common struct
//!

pub(crate) mod bucket;
pub(crate) mod inode;
pub(crate) mod meta;
pub(crate) mod page;
pub(crate) mod types;

use std::mem::align_of;
use std::ptr::{self, NonNull};

pub(crate) use self::bucket::InBucket;
pub(crate) use self::meta::Meta;
pub(crate) use self::page::{Page, PgId, PAGE_HEADER_SIZE};
pub(crate) use self::types::TxId;

// Converts a raw pointer to a pointer offset by a specified amount.
pub unsafe fn unsafe_add<T>(base: *mut T, offset: usize) -> *mut T {
    base.add(offset)
}

// Accesses an element of an array with unsafe pointer arithmetic.
pub unsafe fn unsafe_index<T>(base: *mut T, offset: usize, elemsz: usize, n: usize) -> *mut T {
    base.add(offset).add(n * elemsz)
}

// Creates a slice from a raw pointer with an offset and length.
// WARNING: This function is unsafe and should be used with caution.
pub unsafe fn unsafe_byte_slice<'a>(
    base: *const u8,
    offset: usize,
    i: usize,
    j: usize,
) -> &'a [u8] {
    let slice_ptr = base.add(offset).add(i);
    // let slice_ptr = unsafe_add(base, offset + i);
    std::slice::from_raw_parts(slice_ptr, j - i)
}

// LoadBucket converts a byte slice to an InBucket reference.
pub(crate) unsafe fn load_bucket(buf: &[u8]) -> Option<&InBucket> {
    // &*(buf.as_ptr() as *const InBucket)
    let slice = std::slice::from_raw_parts(buf.as_ptr(), buf.len());

    Some(unsafe { &*(slice.as_ptr() as *const InBucket) })
}

// LoadPage converts a byte slice to a Page reference.
pub(crate) unsafe fn load_page(buf: &[u8]) -> &Page {
    &*(buf.as_ptr() as *const Page)
}

// LoadPageMeta converts a byte slice to a Meta reference.
// Warning: This function is unsafe and should be used with caution.
pub(crate) unsafe fn load_page_meta(buf: &[u8]) -> &Meta {
    let meta_ptr = buf.as_ptr().add(PAGE_HEADER_SIZE);
    &*(meta_ptr as *const Meta)
}

#[allow(dead_code)]
#[inline]
pub(crate) fn must_align<T>(ptr: *const T) {
    let actual = (ptr as usize) % align_of::<T>() == 0;
    assert!(actual);
}
