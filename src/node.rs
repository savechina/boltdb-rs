use crate::bucket::{self, RawBucket, MAX_FILL_PERCENT, MIN_FILL_PERCENT};
use crate::common::inode::Inodes;
use crate::common::page::{Page, PageFlags, MIN_KEYS_PER_PAGE};
use crate::common::page::{
    PgId, BRANCH_PAGE_ELEMENT_SIZE, LEAF_PAGE_ELEMENT_SIZE, PAGE_HEADER_SIZE,
};
use crate::common::types::Byte;
use crate::common::types::Key;
use crate::common::{self, page};
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::io::Read;
use std::ops::{Deref, Index};
use std::ptr::{self, NonNull};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Weak;

use crate::errors::Result;

// Assuming `Bucket`, `common::Pgid`, `common::Inodes`, and `nodes` are defined elsewhere

#[derive(Debug)]
#[repr(C)]
/// A raw node in the B-Tree. This struct represents a page that can be either a branch or a leaf.
// Struct representing an in-memory, deserialized page
pub(crate) struct RawNode<'tx> {
    bucket: *const RawBucket<'tx>, // Use Option<NonNull<T>> for optional non-null pointers
    is_leaf: AtomicBool,
    unbalanced: AtomicBool,
    spilled: AtomicBool,
    key: RefCell<Key>,
    pgid: RefCell<PgId>,
    parent: RefCell<WeakNode<'tx>>, // Use Option<NonNull<T>> for optional non-null pointers
    children: RefCell<Nodes<'tx>>,  // Assuming nodes is already defined
    pub(crate) inodes: RefCell<Inodes>,
}

impl<'tx> RawNode<'tx> {
    fn is_leaf(&self) -> bool {
        self.is_leaf.load(Ordering::Acquire)
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct WeakNode<'tx>(pub(crate) Weak<RawNode<'tx>>);

impl<'tx> WeakNode<'tx> {
    // 创建新的弱引用节点
    pub(crate) fn new() -> Self {
        WeakNode::default()
    }

    // 升级弱引用到强引用
    pub(crate) fn upgrade(&self) -> Option<Node<'tx>> {
        self.0.upgrade().map(Node)
    }

    pub(crate) fn from(tx: &Node<'tx>) -> Self {
        WeakNode(Arc::downgrade(&tx.0))
    }
}

pub(crate) struct NodeCell<'tx>(RefCell<RawNode<'tx>>);

#[derive(Clone, Debug)]
pub(crate) struct Node<'tx>(pub(crate) Arc<RawNode<'tx>>);

impl<'tx> Node<'tx> {
    // Returns the top-level node this node is attached to.
    pub(crate) fn root(&self) -> Node<'tx> {
        match self.parent() {
            Some(ref p) => p.root(),
            None => self.clone(),
        }
    }

    fn parent(&self) -> Option<Node<'tx>> {
        self.0.parent.borrow().upgrade()
    }

    // Returns the minimum number of inodes this node should have.
    pub fn min_keys(&self) -> usize {
        if self.is_leaf() {
            1
        } else {
            2
        }
    }

    pub(crate) fn is_leaf(&self) -> bool {
        self.0.is_leaf()
    }

    // size returns the size of the node after serialization.
    pub fn size(&self) -> usize {
        let mut size = PAGE_HEADER_SIZE;
        size += self.page_element_size();

        let inodes = &self.0.inodes.borrow();

        for inode in inodes.iter() {
            size += self.page_element_size();
            size += inode.key().len() + inode.value().len();
        }

        size
    }

    // func (n *node) sizeLessThan(v uintptr) bool {
    // 	sz, elsz := common.PageHeaderSize, n.pageElementSize()
    // 	for i := 0; i < len(n.inodes); i++ {
    // 		item := &n.inodes[i]
    // 		sz += elsz + uintptr(len(item.Key())) + uintptr(len(item.Value()))
    // 		if sz >= v {
    // 			return false
    // 		}
    // 	}
    // 	return true
    // }

    // sizeLessThan returns true if the node is less than a given size.
    // This is an optimization to avoid calculating a large node when we only need
    // to know if it fits inside a certain page size.
    pub(crate) fn size_less_than(&self, size: usize) -> bool {
        let (mut sz, elsz) = (PAGE_HEADER_SIZE, self.page_element_size());

        let inodes = &self.0.inodes.borrow();

        for inode in inodes.iter() {
            sz += elsz + inode.key().len() + inode.value().len();
            if sz >= size {
                return false;
            }
        }

        true
    }

    // Returns the size of each page element based on type of node.
    fn page_element_size(&self) -> usize {
        if self.is_leaf() {
            LEAF_PAGE_ELEMENT_SIZE
        } else {
            BRANCH_PAGE_ELEMENT_SIZE
        }
    }

    // child_at returns the child node at a given index.
    pub fn child_at(&self, index: usize) -> Result<Node<'tx>> {
        if self.is_leaf() {
            panic!("invalid childAt({}) on a leaf node", index);
        }

        // assert!(!self.is_leaf(), "invalid childAt {} on a leaf node", index);

        let child_pgid = self.0.inodes.borrow().get(index).pgid();

        Ok(self
            .bucket_mut()
            .unwrap()
            .node(child_pgid, WeakNode::from(self)))
    }

    // childIndex returns the index of a given child node.
    pub(crate) fn child_index(&self, child: &Node) -> Option<usize> {
        let key = &child.0.key.borrow();

        let index = self.0.inodes.borrow().binary_search_by(&key).ok();
        index
    }

    // numChildren returns the number of children.
    pub(crate) fn num_children(&self) -> usize {
        self.0.inodes.borrow().len()
    }

    pub(super) fn bucket<'a, 'b: 'a>(&'a self) -> Option<&'tx RawBucket<'tx>> {
        if self.0.bucket.is_null() {
            return None;
        }
        Some(unsafe { &*(self.0.bucket as *const RawBucket) })
    }

    pub(super) fn bucket_mut<'a, 'b: 'a>(&'a self) -> Option<&'tx mut RawBucket<'tx>> {
        if self.0.bucket.is_null() {
            return None;
        }
        Some(unsafe { &mut *(self.0.bucket as *mut RawBucket) })
    }

    // nextSibling returns the next node with the same parent.
    pub(crate) fn next_sibling(&self) -> Option<Node<'tx>> {
        if self.parent().is_none() {
            // No parent, so no sibling
            return None;
        }

        let parent = self.parent().unwrap();

        let index = parent.child_index(self).unwrap();

        if index >= parent.num_children() - 1 {
            // Last child, so no next sibling
            return None;
        }
        // Get the next sibling using child_at
        parent.child_at(index + 1).ok()
    }

    // // next_sibling returns the next node with the same parent.
    // pub fn next_sibling(&self) -> Option<&Node> {
    //     if self.parent.is_none() {
    //         // No parent, so no sibling
    //         return None;
    //     }

    //     let parent = self.parent.as_ref().unwrap();
    //     let index = parent.child_index(self).unwrap();

    //     if index >= parent.num_children() - 1 {
    //         // Last child, so no next sibling
    //         return None;
    //     }

    //     // Get the next sibling using child_at
    //     Some(parent.child_at(index + 1))
    // }

    // prevSibling returns the previous node with the same parent.
    pub(crate) fn prev_sibling(&self) -> Option<Node<'tx>> {
        if self.parent().is_none() {
            return None;
        }
        let parent = self.parent().unwrap();

        let index = parent.child_index(self).unwrap();
        if index == 0 {
            // First child, so no previous sibling

            return None;
        }
        // Get the prev sibling using child_at
        parent.child_at(index - 1).ok()
    }

    // // prev_sibling returns the previous node with the same parent.
    // pub fn prev_sibling(&self) -> Option<&Node> {
    //     if self.parent.is_none() {
    //         // No parent, so no sibling

    //         return None;
    //     }

    //     let parent = self.parent.as_ref().unwrap();
    //     let index = parent.child_index(self).unwrap();

    //     if index == 0 {
    //         // First child, so no previous sibling
    //         return None;
    //     }

    //     Some(parent.child_at(index - 1))
    // }

    /// put inserts a key/value.
    pub(crate) fn put(
        &mut self,
        old_key: &[u8],
        new_key: &[u8],
        value: &[u8],
        pg_id: PgId,
        flags: u32,
    ) {
        //todo
        // assert!(pg_id < self.bucket().unwrap().tx.meta.pgid(),
        //         "pgId ({}) above high water mark ({})",
        //         pg_id, self.bucket().unwrap().tx.meta.pgid());

        assert!(!old_key.is_empty(), "put: zero-length old key");
        assert!(!new_key.is_empty(), "put: zero-length new key");

        let mut inodes = self.0.inodes.borrow_mut();

        // Find insertion index using binary_search_by.
        let index = match inodes.binary_search_by(old_key) {
            Ok(index) => index,
            Err(index) => index, // Position for insertion
        };

        // Shift nodes if needed for insertion.
        if index < inodes.len() && !inodes.get(index).key().eq(old_key) {
            inodes.insert(index, Default::default());
        }

        let inode = inodes.get_mut(index);

        inode.set_flags(flags);
        inode.set_key(new_key.to_vec()); // Assuming key needs to be owned
        inode.set_value(value.to_vec()); // Assuming value needs to be owned
        inode.set_pgid(pg_id);

        assert!(!inode.key().is_empty(), "put: zero-length inode key");
    }

    // // put inserts a key/value.
    // func (n *node) put(oldKey, newKey, value []byte, pgId common.Pgid, flags uint32) {
    // 	if pgId >= n.bucket.tx.meta.Pgid() {
    // 		panic(fmt.Sprintf("pgId (%d) above high water mark (%d)", pgId, n.bucket.tx.meta.Pgid()))
    // 	} else if len(oldKey) <= 0 {
    // 		panic("put: zero-length old key")
    // 	} else if len(newKey) <= 0 {
    // 		panic("put: zero-length new key")
    // 	}

    // 	// Find insertion index.
    // 	index := sort.Search(len(n.inodes), func(i int) bool { return bytes.Compare(n.inodes[i].Key(), oldKey) != -1 })

    // 	// Add capacity and shift nodes if we don't have an exact match and need to insert.
    // 	exact := len(n.inodes) > 0 && index < len(n.inodes) && bytes.Equal(n.inodes[index].Key(), oldKey)
    // 	if !exact {
    // 		n.inodes = append(n.inodes, common.Inode{})
    // 		copy(n.inodes[index+1:], n.inodes[index:])
    // 	}

    // 	inode := &n.inodes[index]
    // 	inode.SetFlags(flags)
    // 	inode.SetKey(newKey)
    // 	inode.SetValue(value)
    // 	inode.SetPgid(pgId)
    // 	common.Assert(len(inode.Key()) > 0, "put: zero-length inode key")
    // }

    /// del removes a key from the node.
    pub(crate) fn del(&mut self, key: &[u8]) {
        // Find index of key.
        let index = match self.0.inodes.borrow().binary_search_by(key) {
            Ok(index) => index,
            // Exit if the key isn't found.
            Err(_) => return, // Key not found
        };

        // Exit if the key isn't found.
        if index >= self.0.inodes.borrow().len()
            || self
                .0
                .inodes
                .borrow()
                .get(index)
                .key()
                .as_slice()
                .cmp(key)
                .is_eq()
        {
            return;
        }

        // Delete inode from the node.
        self.0.inodes.borrow_mut().remove(index);

        // Mark the node as needing rebalancing.
        self.0.unbalanced.store(true, Ordering::Release);
    }

    /// read initializes the node from a page.
    pub(crate) fn read(&mut self, page: &Page) {
        *self.0.pgid.borrow_mut() = page.id();

        self.0.is_leaf.store(page.is_leaf_page(), Ordering::Release);

        let inodes = common::inode::read_inode_from_page(page);

        *self.0.inodes.borrow_mut() = inodes;

        if !(self.0.inodes.borrow().is_empty()) {
            // Save the first key, if any, for parent lookup on spill.
            let key = self
                .0
                .inodes
                .borrow()
                .first()
                .map(|inode| inode.key().clone());

            assert!(
                key.is_none() || key.as_ref().unwrap().len() == 0,
                "read: zero-length node key"
            );

            self.0.key.replace(key.unwrap());
        }
    }

    /// write writes the items onto one or more pages.
    /// The page should have p.id (might be 0 for meta or bucket-inline page) and p.overflow set
    /// and the rest should be zeroed.
    pub(crate) fn write(&self, page: &mut Page) {
        // Assert preconditions
        assert!(
            page.count() == 0 && page.flags().bits() == 0,
            "Node cannot be written to a non-empty page"
        );

        // Initialize page flags
        let flags = match self.is_leaf() {
            true => PageFlags::LEAF_PAGE,
            false => PageFlags::BRANCH_PAGE,
        };

        page.set_flags(flags);

        // Check for inode overflow
        let len = self.0.inodes.borrow().len();
        if len >= u16::MAX as usize {
            panic!("inode overflow: {} (pgid={})", len, page.id());
        }

        // Set page count
        page.set_count(len as u16);

        // Stop here if there are no items to write.
        if page.count() == 0 {
            return;
        }

        // Write inodes to page
        common::inode::write_inode_to_page(self.0.inodes.borrow().deref(), page);

        // Remove debug-only code (n.dump())
    }

    // split breaks up a node into multiple smaller nodes, if appropriate.
    // This should only be called from the spill() function.
    fn split(&mut self, page_size: usize) -> Vec<Node<'tx>> {
        let mut nodes = Vec::new();

        let mut node = self.clone();
        loop {
            // Split node into two.
            let (a, b) = node.split_two(page_size);
            nodes.push(a);

            // If we can't split then exit the loop.
            if b.is_none() {
                break;
            }

            // Set node to b so it gets split on the next iteration.
            node = b.unwrap();
        }

        nodes
    }

    // splitTwo breaks up a node into two smaller nodes, if appropriate.
    // This should only be called from the split() function.
    fn split_two(&mut self, page_size: usize) -> (Node<'tx>, Option<Node<'tx>>) {
        // Ignore the split if the page doesn't have at least enough nodes for
        // two pages or if the nodes can fit in a single page.
        if self.0.inodes.borrow().len() <= (MIN_KEYS_PER_PAGE * 2) as usize
            || self.size_less_than(page_size)
        {
            return (self.clone(), None);
        }

        // Determine the threshold before starting a new node.
        let clamp = |n, min, max| -> f64 {
            if n < min {
                return min;
            } else if n > max {
                return max;
            }

            return n;
        };

        // Calculate fill threshold.
        let fill_percent = clamp(
            self.bucket().unwrap().fill_percent,
            MIN_FILL_PERCENT,
            MAX_FILL_PERCENT,
        );

        let threshold = (page_size as f64 * fill_percent) as usize;

        // Determine split position and sizes of the two pages.
        let split_index = self.split_index(threshold).0; // Assuming split_index returns Option

        // Split node into two separate nodes.
        // If there's no parent then we'll need to create one.

        if self.parent().is_none() {
            let mut v = Vec::new();
            v.push(self.clone());

            let parent = NodeBuilder::new(self.0.bucket).children(v).build();

            *self.0.parent.borrow_mut() = WeakNode::from(&parent);
        }

        // Create a new node and add it to the parent.
        // Create a new node.

        let next = NodeBuilder::new(self.0.bucket)
            .is_leaf(self.is_leaf())
            .build();

        // Add new node to parent.
        // todo add to parent's children
        // self.parent()
        // .unwrap()
        // .0
        // .children
        // .borrow_mut()
        // .push(next);

        // Split inodes across two nodes. split off modify origin node's inodes at index
        *next.0.inodes.borrow_mut() = self.0.inodes.borrow_mut().split_off(split_index);

        // Update the statistics.
        // self.bucket().tx.stats.inc_split(1);

        (self.clone(), Some(next)) // Return both nodes as an Option
    }

    fn split_index(&self, threshold: usize) -> (usize, usize) {
        let mut sz = page::PAGE_HEADER_SIZE;
        let mut index = 0;

        // Loop until minimum keys remain for the second page.
        for i in 0..self.0.inodes.borrow().len() - MIN_KEYS_PER_PAGE as usize {
            // Calculate element size.
            let elsize = self.page_element_size()
                + self.0.inodes.borrow().inodes[i].key().len()
                + self.0.inodes.borrow().inodes[i].value().len();

            // Check for split condition.
            if i >= MIN_KEYS_PER_PAGE as usize && sz + elsize > threshold {
                break;
            }

            // Update size and index.
            sz += elsize;
            index = i;
        }

        (index, sz)
    }

    // removes a node from the list of in-memory children.
    // This does not affect the inodes.
    fn remove_child(&mut self, target: &Node<'tx>) {
        //可能有性能问题
        self.0.children.borrow_mut().retain(target);
    }
}

pub(crate) struct NodeBuilder<'tx> {
    bucket: Option<*const RawBucket<'tx>>,
    is_leaf: bool,
    pg_id: PgId,
    parent: WeakNode<'tx>,
    children: Nodes<'tx>,
}

impl<'tx> NodeBuilder<'tx> {
    pub(crate) fn new(bucket: *const RawBucket<'tx>) -> Self {
        NodeBuilder {
            bucket: Some(bucket),
            is_leaf: false,
            pg_id: 0,
            parent: Default::default(),
            children: Nodes { inner: vec![] },
        }
    }

    pub(crate) fn is_leaf(mut self, value: bool) -> Self {
        self.is_leaf = value;
        self
    }

    pub(crate) fn parent(mut self, value: WeakNode<'tx>) -> Self {
        self.parent = value;
        self
    }

    pub(crate) fn children(mut self, value: Vec<Node<'tx>>) -> Self {
        self.children = Nodes { inner: value };
        self
    }

    fn bucket(mut self, value: *const RawBucket<'tx>) -> Self {
        self.bucket = Some(value);
        self
    }

    pub(crate) fn build(self) -> Node<'tx> {
        Node(Arc::new(RawNode {
            bucket: self.bucket.unwrap(),
            is_leaf: AtomicBool::new(self.is_leaf),
            spilled: AtomicBool::new(false),
            unbalanced: AtomicBool::new(false),
            key: RefCell::new(vec![]),
            pgid: RefCell::new(self.pg_id),
            parent: RefCell::new(self.parent),
            children: RefCell::new(self.children),
            inodes: RefCell::new(Inodes::default()),
        }))
    }
}

#[derive(Debug)]
pub(crate) struct Nodes<'tx> {
    inner: Vec<Node<'tx>>,
}

impl<'tx> Nodes<'tx> {
    fn retain(&mut self, target: &Node<'tx>) {
        self.inner.retain(|child| !(ptr::eq(child, target)));
    }

    fn push(&mut self, value: Node<'tx>) {
        self.inner.push(value);
    }

    fn split_off(&mut self, index: usize) -> Self {
        let other = self.inner.split_off(index);

        Self { inner: other }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_nodes() {
        let mut nodes = Nodes {
            inner: vec![
                NodeBuilder::new(ptr::null()).build(),
                NodeBuilder::new(ptr::null()).build(),
            ],
        };
        dbg!(&nodes);
    }

    #[test]
    fn test_nodes_retain() {
        let mut nodes = Nodes {
            inner: vec![
                NodeBuilder::new(ptr::null()).build(),
                NodeBuilder::new(ptr::null()).build(),
            ],
        };
    }
}
