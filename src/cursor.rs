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
use crate::common::page;
use crate::common::page::Page;
use crate::common::types::Key;
use crate::common::types::Value;
use crate::node::Node;

struct Cursor<'tx> {
    bucket: &'tx Bucket, // Reference to the bucket with lifetime bound
    stack: RefCell<Vec<ElemRef<'tx>>>,
}

trait CursorApi {
    fn first(&mut self) -> (Key, Value);
    fn last(&mut self) -> (Key, Value);
    fn next(&mut self) -> (Key, Value);
    fn prev(&mut self) -> (Key, Value);
    fn seek(&mut self, k: &Key) -> (Key, Value);
    fn delete(&mut self);
}

impl<'tx> CursorApi for Cursor<'tx> {
    fn first(&mut self) -> (Key, Value) {
        let (key, value, flags) = self.api_first();

        match (flags & page::BUCKET_LEAF_FLAG) != 0 {
            true => return (key, value),
            false => (),
        }
        todo!()
    }

    fn last(&mut self) -> (Key, Value) {
        todo!()
    }

    fn next(&mut self) -> (Key, Value) {
        todo!()
    }

    fn prev(&mut self) -> (Key, Value) {
        todo!()
    }

    fn seek(&mut self, k: &Key) -> (Key, Value) {
        todo!()
    }

    fn delete(&mut self) {
        todo!()
    }
}

impl<'tx> Cursor<'tx> {
    // Bucket returns the bucket that this cursor was created from.
    pub fn bucket(&self) -> &'tx Bucket {
        self.bucket
    }

    fn api_first(&self) -> (Vec<u8>, Vec<u8>, u32) {
        // Clear the stack
        self.stack.borrow_mut().clear();

        // Get the root page and node
        let (p, n) = self.bucket.page_node(self.bucket.root_page());
        self.stack.borrow_mut().push(ElemRef {
            page: Some(p),
            node: Some(n),
            index: 0,
        });

        // Go to the first element on the stack
        self.go_to_first_element_on_the_stack();

        // If we land on an empty page then move to the next value.
        // https://github.com/boltdb/bolt/issues/450
        if self.stack.borrow().last().unwrap().count() == 0 {
            self.next();
        }

        let (k, v, flags) = self.key_value();
        match (flags & page::BUCKET_LEAF_FLAG) != 0 {
            true => return (k, vec![], flags),
            false => (),
        }
        (k, v, flags)
    }

    fn next(&self) -> (Vec<u8>, Vec<u8>, u32) {
        todo!()
    }

    fn last(&self) {
        todo!()
    }

    fn go_to_first_element_on_the_stack(&self) -> () {
        todo!()
    }

    fn key_value(&self) -> (Vec<u8>, Vec<u8>, u32) {
        todo!()
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
    use crate::common::{page::PageFlags, PgId};

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
