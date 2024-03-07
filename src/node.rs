use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bucket::Bucket;
use crate::common;
use crate::common::inode::{Inode, Inodes, Key};
use crate::common::page::PgId;
use crate::common::types::Byte;

// Assuming `Bucket`, `common::Pgid`, `common::Inodes`, and `nodes` are defined elsewhere

#[derive(Debug)]
// Struct representing an in-memory, deserialized page
pub(crate) struct RawNode {
    pub bucket: *const Bucket, // Use Option<NonNull<T>> for optional non-null pointers
    pub is_leaf: AtomicBool,
    pub unbalanced: AtomicBool,
    pub spilled: AtomicBool,
    pub key: RefCell<Key>,
    pub pgid: RefCell<PgId>,
    pub parent: RefCell<WeakNode>, // Use Option<NonNull<T>> for optional non-null pointers
    pub children: RefCell<Nodes>,  // Assuming nodes is already defined
    pub inodes: RefCell<Inodes>,
}

impl RawNode {


    // Returns the minimum number of inodes this node should have.
    pub fn min_keys(&self) -> usize {
        if self.is_leaf() {
            1
        } else {
            2
        }
    }

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

    
}

#[derive(Debug)]
pub(crate) struct Nodes {
    pub(crate) nodes: Vec<RawNode>,
}
