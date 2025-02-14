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
use crate::common::types::Byte;
use crate::common::types::Bytes;
use crate::common::types::Key;
use crate::common::types::Value;
use crate::common::PgId;
use crate::node::Node;

struct Cursor<'tx> {
    bucket: &'tx Bucket, // Reference to the bucket with lifetime bound
    stack: RefCell<Vec<ElemRef<'tx>>>,
}

trait CursorApi {
    fn first(&mut self) -> Option<(&Bytes, Option<&Bytes>)>;
    // fn first(&mut self) -> (Key, Value);
    fn last(&mut self) -> Option<(&Bytes, Option<&Bytes>)>;
    fn next(&mut self) -> Option<(&Bytes, Option<&Bytes>)>;
    fn prev(&mut self) -> Option<(&Bytes, Option<&Bytes>)>;
    fn seek(&mut self, seek: &Bytes) -> Option<(&Bytes, Option<&Bytes>)>;
    fn delete(&mut self) -> crate::Result<()>; // Return Result for error handling
}

impl<'tx> CursorApi for Cursor<'tx> {
    fn first(&mut self) -> Option<(&'tx Bytes, Option<&'tx Bytes>)> {
        let (key, value, flags) = self.raw_first();

        match (flags & page::BUCKET_LEAF_FLAG) != 0 {
            true => return Some((key, Some(value))),
            false => return Some((key, None)),
        }
    }

    fn last(&mut self) -> Option<(&'tx Bytes, Option<&'tx Bytes>)> {
        let (key, value, flags) = self.raw_last();

        match (flags & page::BUCKET_LEAF_FLAG) != 0 {
            true => return Some((key, Some(value))),
            false => return Some((key, None)),
        }
    }

    fn next(&mut self) -> Option<(&'tx Bytes, Option<&'tx Bytes>)> {
        let (key, value, flags) = self.raw_next();

        match (flags & page::BUCKET_LEAF_FLAG) != 0 {
            true => return Some((key, Some(value))),
            false => return Some((key, None)),
        }
    }

    fn prev(&mut self) -> Option<(&'tx Bytes, Option<&'tx Bytes>)> {
        let (key, value, flags) = self.raw_prev();

        match (flags & page::BUCKET_LEAF_FLAG) != 0 {
            true => return Some((key, Some(value))),
            false => return Some((key, None)),
        }
    }

    fn seek(&mut self, k: &[u8]) -> Option<(&'tx Bytes, Option<&'tx Bytes>)> {
        let (key, value, flags) = self.raw_seek(k);

        match (flags & page::BUCKET_LEAF_FLAG) != 0 {
            true => return Some((key, Some(value))),
            false => return Some((key, None)),
        }
    }

    fn delete(&mut self) -> crate::Result<()> {
        // self.stack.borrow_mut().last().unwrap();
        // Ok(())
        return self.raw_delete();
    }
}

impl<'tx> Cursor<'tx> {
    // Bucket returns the bucket that this cursor was created from.
    pub fn bucket(&self) -> &'tx Bucket {
        self.bucket
    }

    fn raw_first(&self) -> (&'tx Bytes, &'tx Bytes, u32) {
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
            self.raw_next();
        }

        let (k, v, flags) = self.key_value();

        match (flags & page::BUCKET_LEAF_FLAG) != 0 {
            true => return (k, &[], flags),
            false => (),
        }

        (k, v, flags)
    }

    fn raw_next(&self) -> (&'tx Bytes, &'tx Bytes, u32) {
        todo!()
    }

    fn raw_prev(&self) -> (&'tx Bytes, &'tx Bytes, u32) {
        todo!()
    }

    fn raw_last(&self) -> (&'tx Bytes, &'tx Bytes, u32) {
        todo!()
    }

    fn raw_seek(&mut self, k: &[u8]) -> (&'tx Bytes, &'tx Bytes, u32) {
        todo!()
    }

    fn go_to_first_element_on_the_stack(&self) -> () {
        todo!()
    }

    fn search(&mut self, pgId: PgId) -> () {
        todo!()
    }

    fn nsearch(&mut self, key: &[u8]) {
        todo!()
    }

    fn key_value(&self) -> (&'tx Bytes, &'tx Bytes, u32) {
        todo!()
    }

    fn raw_delete(&self) -> crate::Result<()> {
        todo!()
    }
}

struct CursorIter<'tx> {
    cursor: Cursor<'tx>,
}

trait CursorIterApi {
    fn next(&mut self) -> Option<(Key, Option<Value>)>;
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
