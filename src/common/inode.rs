use std::cmp::Ordering;
use std::result::Result;
use std::slice::Iter;
use std::vec::IntoIter;

use crate::common::page::{BranchPageElement, LeafPageElement, Page, PgId};
use crate::common::types::Byte;

//Key 字节数组
pub(crate) type Key = Vec<Byte>;

//Value 字节数组
pub(crate) type Value = Vec<Byte>;

/// Inode 结构体
/// Inode represents an internal node inside of a node.
/// It can be used to point to elements in a page or point
/// to an element which hasn't been added to a page yet.
#[derive(Debug, Default, Clone)]
#[repr(C)] // 确保 C 兼容的内存布局
pub(crate) struct Inode {
    flags: u32,
    pgid: PgId,
    key: Key,
    value: Value,
}

impl Inode {
    pub(crate) fn flags(&self) -> u32 {
        self.flags
    }
    pub(crate) fn flags_mut(&mut self) -> &mut u32 {
        &mut self.flags
    }
    pub(crate) fn set_flags(&mut self, flags: u32) {
        self.flags = flags;
    }

    pub(crate) fn key(&self) -> &Key {
        &self.key
    }

    pub(crate) fn set_key(&mut self, key: Key) {
        self.key = key;
    }

    pub(crate) fn value(&self) -> &Value {
        &self.value
    }

    pub(crate) fn set_value(&mut self, value: Value) {
        self.value = value;
    }

    pub(crate) fn pgid(&self) -> PgId {
        self.pgid
    }

    pub(crate) fn set_pgid(&mut self, pgid: PgId) {
        self.pgid = pgid;
    }
}

#[derive(Default, Debug)]
pub(crate) struct Inodes {
    pub(crate) inodes: Vec<Inode>,
}

impl Inodes {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.inodes.len()
    }

    #[inline]
    pub(crate) fn get(&self, index: usize) -> &Inode {
        &self.inodes[index]
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, index: usize) -> &mut Inode {
        &mut self.inodes[index]
    }

    #[inline]
    pub(crate) fn first(&self) -> Option<&Inode> {
        self.inodes.first()
    }

    #[inline]
    pub(crate) fn first_mut(&mut self) -> Option<&mut Inode> {
        self.inodes.first_mut()
    }

    #[inline]
    pub(crate) fn insert(&mut self, index: usize, inode: Inode) {
        self.inodes.insert(index, inode);
    }

    #[inline]
    pub(crate) fn push(&mut self, inode: Inode) {
        self.inodes.push(inode);
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.inodes.is_empty()
    }

    #[inline]
    pub(crate) fn remove(&mut self, index: usize) {
        self.inodes.remove(index);
    }

    #[inline]
    pub(crate) fn iter(&self) -> Iter<'_, Inode> {
        self.inodes.iter()
    }

    #[inline]
    pub(crate) fn binary_search_by(&self, key: &[u8]) -> Result<usize, usize> {
        self.inodes
            .binary_search_by(|node| node.key.as_slice().cmp(key))
    }

    #[inline]
    pub(crate) fn as_slice(&self) -> &Vec<Inode> {
        &self.inodes
    }
}

/// Assuming necessary struct and trait definitions for Inode, Page, etc.
// Initializes the node from a page.
pub(crate) fn read_inode_from_page(page: &Page) -> Inodes {
    //TODO: rewrite handle write Inode to Page   2024/03/05

    let mut inodes = Vec::with_capacity(page.count() as usize);

    let is_leaf = page.is_leaf_page();

    for i in 0..page.count() as usize {
        let mut inode = Inode::default(); // Use a default Inode instance

        if is_leaf {
            let elem = page.leaf_page_element(i);
            inode.set_flags(elem.flags());
            inode.set_key(Vec::from(elem.key()));
            inode.set_value(Vec::from(elem.value()));
        } else {
            let elem = page.branch_page_element(i);

            inode = Inode {
                flags: 0,
                pgid: elem.pgid(),
                key: Vec::from(elem.key()),
                value: Vec::new(),
            };

            inode.pgid = elem.pgid();
            inode.key = Vec::from(elem.key());
        }

        assert!(inode.key.len() > 0, "read: zero-length inode key");
        inodes.push(inode);
    }

    Inodes { inodes: inodes }

}

// Writes the items onto one or more pages.
pub(crate) fn write_inode_to_page(inodes: &Inodes, page: &mut Page) -> u32 {
    //TODO: rewrite handle write Inode to Page   2024/03/05

    // Loop over each item and write it to the page.
    // off tracks the offset into p of the start of the next data.
    let mut offset: usize = page.page_element_size() as usize * inodes.len();

    let data_ptr = unsafe { page.get_data_mut_ptr().add(offset) };

    let is_leaf = page.is_leaf_page();

    for (i, item) in inodes.iter().enumerate() {
        assert!(item.key().len() > 0, "write: zero-length inode key");

        // Create a slice to write into of needed size and advance
        // byte pointer for next iteration.
        let size = item.key().len() + item.value().len();

        let mut data_slice: &[u8] = unsafe { page.get_data_slice() }; // Use as_mut_slice() for safe access

        offset += size;

        // Write the page element.
        if is_leaf {
            let mut elem: &mut LeafPageElement = page.leaf_page_element_mut(i);
            let elem_ptr = elem as *const LeafPageElement as *const u8;

            &elem.set_pos(unsafe { data_ptr.sub(elem_ptr as usize) as u32 });
            elem.set_flags(item.flags() as u32);
            elem.set_ksize(item.key().len() as u32);
            elem.set_vsize(item.value().len() as u32);
        } else {
            let mut elem = page.branch_page_element_mut(i);
            let elem_ptr = elem as *const BranchPageElement as *const u8;

            elem.set_pos(unsafe { data_ptr.sub(elem_ptr as usize) as u32 });
            elem.set_ksize(item.key().len() as u32);
            elem.set_pgid(item.pgid());

            assert!(
                elem.pgid() != page.id(),
                "write: circular dependency occurred"
            );
        }

        todo!();

        let key_len = item.key().len();

        data_slice[..key_len].copy_from_slice(item.key());
        data_slice[key_len..].copy_from_slice(item.value().as_slice());
    }

    offset as u32
}

/*
fn used_space_in_page(inodes: &[Inode], page: &Page) -> u32 {
    let mut offset = page.size_of() + page.page_element_size() as usize * inodes.len();
    for item in inodes {
        offset += item.key().len() + item.value().len();
    }

    offset as u32
} */
