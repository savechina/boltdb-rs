// Cursor represents an iterator that can traverse over all key/value pairs in a bucket
// in lexicographical order.
// Cursors see nested buckets with value == nil.
// Cursors can be obtained from a transaction and are valid as long as the transaction is open.
//
// Keys and values returned from the cursor are only valid for the life of the transaction.
//
// Changing data while traversing with a cursor may cause it to be invalidated
// and return unexpected keys and/or values. You must reposition your cursor
// after mutating data.

use std::cell::RefCell;

use crate::bucket::Bucket;
use crate::common::page::Page;
use crate::node::Node;

struct Cursor<'a> {
    bucket: &'a Bucket, // Reference to the bucket with lifetime bound
    stack: RefCell<Vec<ElemRef<'a>>>,
}

impl<'a> Cursor<'a> {
    // Bucket returns the bucket that this cursor was created from.
    pub fn bucket(&self) -> &'a Bucket {
        self.bucket
    }
}

// elemRef represents a reference to an element on a given page/node.
// This is used to track the current position of the cursor during iteration.
#[derive(Debug)]
struct ElemRef<'a> {
    page: Option<&'a Page>, // Option for handling potential nil pages
    node: Option<&'a Node>, // Option for handling potential nil nodes
    index: usize,           // Use usize for memory-related integer
}

impl<'a> ElemRef<'a> {
    // isLeaf returns whether the ref is pointing at a leaf page/node.
    fn is_leaf(&self) -> bool {
        if self.node.is_some() {
            let is_leaf = self.node.map_or(false, |n| n.is_leaf());
            return is_leaf;
        }

        //assert is page
        if self.page.is_none() {
            panic!("ElemRef not page")
        }

        self.page.unwrap().is_leaf_page()
    }

    fn count(&self) -> usize {
        self.node.map_or(0, |n| n.0.inodes.borrow().len()) // Use map_or for optional counting
    }
}

#[cfg(test)]
mod tests {
    use crate::common::page::PageFlags;

    use super::*;
    #[test]
    fn test_elem_ref() {
        // Create a new page with branch page flags
        let branch_page = Page::new(0, PageFlags::BRANCH_PAGE, 0, 0);

        // Create a new element reference with the branch page
        let mut elem = ElemRef {
            page: Some(&branch_page),
            node: None,
            index: 0,
        };

        dbg!(&elem);
    }
}
