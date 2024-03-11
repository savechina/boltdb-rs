use crate::bucket::Bucket;
use crate::common;
use crate::common::inode::{Inode, Inodes, Key};
use crate::common::page::Page;
use crate::common::page::{
    PgId, BRANCH_PAGE_ELEMENT_SIZE, LEAF_PAGE_ELEMENT_SIZE, PAGE_HEADER_SIZE,
};
use crate::common::types::Byte;
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::ops::Deref;
use std::ptr::NonNull;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::errors::Result;

// Assuming `Bucket`, `common::Pgid`, `common::Inodes`, and `nodes` are defined elsewhere

#[derive(Debug)]
// Struct representing an in-memory, deserialized page
pub(crate) struct RawNode {
    bucket: *const Bucket, // Use Option<NonNull<T>> for optional non-null pointers
    is_leaf: AtomicBool,
    unbalanced: AtomicBool,
    spilled: AtomicBool,
    key: RefCell<Key>,
    pgid: RefCell<PgId>,
    parent: RefCell<WeakNode>, // Use Option<NonNull<T>> for optional non-null pointers
    children: RefCell<Nodes>,  // Assuming nodes is already defined
    inodes: RefCell<Inodes>,
}

impl RawNode {
    fn is_leaf(&self) -> bool {
        self.is_leaf.load(Ordering::Acquire)
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct WeakNode(pub(crate) Weak<RawNode>);

impl WeakNode {
    // 创建新的弱引用节点
    pub(crate) fn new() -> Self {
        WeakNode::default()
    }

    // 升级弱引用到强引用
    pub(crate) fn upgrade(&self) -> Option<Node> {
        self.0.upgrade().map(Node)
    }

    pub(crate) fn from(tx: &Node) -> Self {
        WeakNode(Rc::downgrade(&tx.0))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Node(pub(crate) Rc<RawNode>);

impl Node {
    // Returns the top-level node this node is attached to.
    pub(crate) fn root(&self) -> Node {
        match self.parent() {
            Some(ref p) => p.root(),
            None => self.clone(),
        }
    }

    fn parent(&self) -> Option<Node> {
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
    pub fn child_at(&self, index: usize) -> Result<Node> {
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

    pub(super) fn bucket<'a, 'b: 'a>(&'a self) -> Option<&'b Bucket> {
        if self.0.bucket.is_null() {
            return None;
        }
        Some(unsafe { &*(self.0.bucket as *const Bucket) })
    }

    pub(super) fn bucket_mut<'a, 'b: 'a>(&'a self) -> Option<&'b mut Bucket> {
        if self.0.bucket.is_null() {
            return None;
        }
        Some(unsafe { &mut *(self.0.bucket as *mut Bucket) })
    }

    // nextSibling returns the next node with the same parent.
    pub(crate) fn next_sibling(&self) -> Option<Node> {
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
    pub(crate) fn prev_sibling(&self) -> Option<Node> {
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
        // if index >= self.0.inodes.borrow().len() || self.0.inodes.borrow().get(index).key().as_slice().cmp(key).is_eq() {
        //     return;
        // }

        // Delete inode from the node.
        self.0.inodes.borrow_mut().remove(index);

        // Mark the node as needing rebalancing.
        self.0.unbalanced.store(true, Ordering::Release);
    }

    /// read initializes the node from a page.
    pub(crate) fn read(&mut self, page: &Page) {
        *self.0.pgid.borrow_mut() = page.id();

        self.0.is_leaf.store(page.is_leaf_page(), Ordering::Release);

        let indoes = common::inode::read_inode_from_page(page);

        *self.0.inodes.borrow_mut() = Inodes::default();

        // Save the first key, if any, for parent lookup on spill.
        let key = indoes.first().map(|inode| inode.key()).cloned();
        //    *self.0.key.borrow_mut() ;
        assert!(
            key.is_none() || key.as_ref().unwrap().len() > 0,
            "read: zero-length node key"
        );
    }
}

#[derive(Debug)]
pub(crate) struct Nodes {
     inner: Vec<RawNode>,
}
