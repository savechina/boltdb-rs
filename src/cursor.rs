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
use crate::common::page::OwnedPage;
use crate::common::page::Page;
use crate::common::types::Bytes;
use crate::common::PgId;
use crate::node::Node;

pub trait CursorApi<'tx> {
    /// First moves the cursor to the first item in the bucket and returns its key and value.
    /// If the bucket is empty then a nil key and value are returned.
    /// The returned key and value are only valid for the life of the transaction.
    fn first(&mut self) -> Option<Entry<'tx>>;

    /// Last moves the cursor to the last item in the bucket and returns its key and value.
    /// If the bucket is empty then a nil key and value are returned.
    /// The returned key and value are only valid for the life of the transaction.
    fn last(&mut self) -> Option<Entry<'tx>>;

    /// Next moves the cursor to the next item in the bucket and returns its key and value.
    /// If the cursor is at the end of the bucket then a nil key and value are returned.
    /// The returned key and value are only valid for the life of the transaction.
    fn next(&mut self) -> Option<Entry<'tx>>;

    /// Prev moves the cursor to the previous item in the bucket and returns its key and value.
    /// If the cursor is at the beginning of the bucket then a nil key and value are returned.
    /// The returned key and value are only valid for the life of the transaction.
    fn prev(&mut self) -> Option<Entry<'tx>>;

    /// Seek moves the cursor to a given key using a b-tree search and returns it.
    /// If the key does not exist then the next key is used. If no keys
    /// follow, a nil key is returned.
    /// The returned key and value are only valid for the life of the transaction.
    fn seek(&mut self, seek: &Bytes) -> Option<Entry<'tx>>;

    /// Delete removes the current key/value under the cursor from the bucket.
    /// Delete fails if current key/value is a bucket or if the transaction is not writable.
    fn delete(&mut self) -> crate::Result<()>; // Return Result for error handling
}

pub struct Cursor<'tx> {
    raw: RefCell<RawCursor<'tx>>,
}

impl<'tx> CursorApi<'tx> for Cursor<'tx> {
    /// First moves the cursor to the first item in the bucket and returns its key and value.
    /// If the bucket is empty then a nil key and value are returned.
    /// The returned key and value are only valid for the life of the transaction.
    fn first(&mut self) -> Option<Entry<'tx>> {
        let raw_entry = self.raw.borrow().raw_first();

        match raw_entry {
            Some(entry) => {
                if (entry.flags & page::BUCKET_LEAF_FLAG) != 0 {
                    return Some(Entry {
                        key: entry.key,
                        value: None,
                    });
                }
                return Some(Entry {
                    key: entry.key,
                    value: Some(entry.value),
                });
            }
            None => None,
        }
    }

    fn last(&mut self) -> Option<Entry<'tx>> {
        let raw_entry = self.raw.borrow().raw_last();

        match raw_entry {
            Some(entry) => {
                if (entry.flags & page::BUCKET_LEAF_FLAG) != 0 {
                    return Some(Entry {
                        key: entry.key,
                        value: None,
                    });
                }
                return Some(Entry {
                    key: entry.key,
                    value: Some(entry.value),
                });
            }
            None => None,
        }
    }

    fn next(&mut self) -> Option<Entry<'tx>> {
        let raw_entry = self.raw.borrow().raw_next();

        match raw_entry {
            Some(entry) => {
                if (entry.flags & page::BUCKET_LEAF_FLAG) != 0 {
                    return Some(Entry {
                        key: entry.key,
                        value: None,
                    });
                }
                return Some(Entry {
                    key: entry.key,
                    value: Some(entry.value),
                });
            }
            None => None,
        }
        // match (entry.flags & page::BUCKET_LEAF_FLAG) != 0 {
        //     true => return Some((key, Some(value))),
        //     false => return Some((key, None)),
        // }
    }

    fn prev(&mut self) -> Option<Entry<'tx>> {
        let raw_entry = self.raw.borrow().raw_prev();

        match raw_entry {
            Some(entry) => {
                if (entry.flags & page::BUCKET_LEAF_FLAG) != 0 {
                    return Some(Entry {
                        key: entry.key,
                        value: None,
                    });
                }
                return Some(Entry {
                    key: entry.key,
                    value: Some(entry.value),
                });
            }
            None => None,
        }
    }

    fn seek(&mut self, k: &[u8]) -> Option<Entry<'tx>> {
        let raw_entry = self.raw.borrow_mut().raw_seek(k);

        match raw_entry {
            Some(entry) => {
                if (entry.flags & page::BUCKET_LEAF_FLAG) != 0 {
                    return Some(Entry {
                        key: entry.key,
                        value: None,
                    });
                }
                return Some(Entry {
                    key: entry.key,
                    value: Some(entry.value),
                });
            }
            None => None,
        }
    }

    fn delete(&mut self) -> crate::Result<()> {
        // self.stack.borrow_mut().last().unwrap();
        // Ok(())
        return self.raw.borrow().raw_delete();
    }
}

pub(crate) trait RawCursorApi<'tx> {
    /// Bucket returns the bucket that this cursor was created from.
    fn bucket(&self) -> &'tx Bucket;

    /// First moves the cursor to the first item in the bucket and returns its key and value.
    /// If the bucket is empty then a nil key and value are returned.
    /// The returned key and value are only valid for the life of the transaction.
    fn raw_first(&self) -> Option<RawEntry<'tx>>;

    /// next moves to the next leaf element and returns the key and value.
    /// If the cursor is at the last leaf element then it stays there and returns nil.
    fn raw_next(&self) -> Option<RawEntry<'tx>>;

    /// prev moves the cursor to the previous item in the bucket and returns its key and value.
    /// If the cursor is at the beginning of the bucket then a nil key and value are returned.
    fn raw_prev(&self) -> Option<RawEntry<'tx>>;

    /// last moves the cursor to the last leaf element under the last page in the stack.
    fn raw_last(&self) -> Option<RawEntry<'tx>>;

    /// seek moves the cursor to a given key and returns it.
    /// If the key does not exist then the next key is used.
    fn raw_seek(&mut self, k: &[u8]) -> Option<RawEntry<'tx>>;

    /// first moves the cursor to the first leaf element under the last page in the stack.
    fn go_to_first_element_on_the_stack(&self) -> ();

    /// search recursively performs a binary search against a given page/node until it finds a given key.
    fn search(&mut self, seek: &[u8], pgId: PgId) -> ();

    /// nsearch searches the leaf node on the top of the stack for a key.
    fn nsearch(&mut self, key: &[u8]);

    fn search_node(&mut self, key: &[u8], node: &Node<'tx>);

    fn search_page(&mut self, key: &[u8], page: &Page);

    fn key_value(&self) -> Option<RawEntry<'tx>>;

    fn raw_delete(&self) -> crate::Result<()>;

    /// node returns the node that the cursor is currently positioned on.
    fn node(&mut self) -> RefCell<Node<'tx>>;
}

struct RawCursor<'tx> {
    bucket: &'tx Bucket<'tx>,
    stack: RefCell<Vec<ElemRef<'tx>>>,
}

impl<'tx> RawCursorApi<'tx> for RawCursor<'tx> {
    // Bucket returns the bucket that this cursor was created from.
    fn bucket(&self) -> &'tx Bucket {
        self.bucket
    }

    fn raw_first(&self) -> Option<RawEntry<'tx>> {
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

        return self.key_value();
    }

    /// next moves to the next leaf element and returns the key and value.
    /// If the cursor is at the last leaf element then it stays there and returns nil.
    fn raw_next(&self) -> Option<RawEntry<'tx>> {
        loop {
            // Attempt to move over one element until we're successful.
            // Move up the stack as we hit the end of each page in our stack.
            let mut new_stack_depth = 0;
            let mut stack_exhausted = true;

            let mut stack = self.stack.borrow_mut();

            for (depth, elem) in stack.iter_mut().enumerate().rev() {
                new_stack_depth = depth + 1;
                if elem.index < elem.count() - 1 {
                    elem.index += 1;
                    stack_exhausted = false;
                    break;
                }
            }

            // If we've hit the root page then stop and return. This will leave the
            // cursor on the last element of the last page.
            if stack_exhausted {
                return None;
            }

            stack.pop();

            stack.truncate(new_stack_depth);

            self.go_to_first_element_on_the_stack();

            return self.key_value();
        }
    }

    // prev moves the cursor to the previous item in the bucket and returns its key and value.
    // If the cursor is at the beginning of the bucket then a nil key and value are returned.
    fn raw_prev(&self) -> Option<RawEntry<'tx>> {
        todo!()
    }

    /// last moves the cursor to the last leaf element under the last page in the stack.
    fn raw_last(&self) -> Option<RawEntry<'tx>> {
        todo!()
    }

    /// seek moves the cursor to a given key and returns it.
    /// If the key does not exist then the next key is used.
    fn raw_seek(&mut self, seek: &[u8]) -> Option<RawEntry<'tx>> {
        // Start from root page/node and traverse to correct page.
        // self.stack.borrow_mut().clear();
        self.stack.borrow_mut().truncate(0);

        self.search(seek, self.bucket.root());

        // If this is a bucket then return a nil value.
        return self.key_value();
    }

    // first moves the cursor to the first leaf element under the last page in the stack.
    fn go_to_first_element_on_the_stack(&self) -> () {
        loop {
            let stack = self.stack.borrow_mut();

            let r = stack.last().unwrap();

            if r.is_leaf() {
                break;
            }

            let pgid: PgId = {
                if let Some(node) = &r.node {
                    let inode = node.0.inodes.borrow();
                    let elem = inode.get(r.index as usize);
                    elem.pgid()
                } else if let Some(page) = &r.page {
                    page.branch_page_element(r.index as usize).pgid()
                } else {
                    panic!("ElemRef not page or node")
                }
            };

            let page_node = self.bucket.page_node(pgid);

            self.stack.borrow_mut().push(ElemRef {
                page: Some(page_node.0),
                node: Some(page_node.1),
                index: 0,
            });
        }
    }

    /// search recursively performs a binary search against a given page/node until it finds a given key.
    fn search(&mut self, seek: &[u8], pgId: PgId) -> () {
        todo!()
    }

    fn nsearch(&mut self, key: &[u8]) {
        todo!()
    }

    fn search_node(&mut self, key: &[u8], node: &Node<'tx>) {
        todo!()
    }

    fn search_page(&mut self, key: &[u8], page: &Page) {
        todo!()
    }

    fn key_value(&self) -> Option<RawEntry<'tx>> {
        todo!()
    }

    fn raw_delete(&self) -> crate::Result<()> {
        todo!()
    }

    /// node returns the node that the cursor is currently positioned on.
    fn node(&mut self) -> RefCell<Node<'tx>> {
        todo!()
    }
}

struct Entry<'tx> {
    key: &'tx Bytes,
    value: Option<&'tx Bytes>,
}

struct RawEntry<'tx> {
    pub(crate) key: &'tx Bytes,
    pub(crate) value: &'tx Bytes,
    pub(crate) flags: u32,
}

enum PageNode<'tx> {
    Page(Page),
    Node(Node<'tx>),
}

// elemRef represents a reference to an element on a given page/node.
// This is used to track the current position of the cursor during iteration.
#[derive(Debug, Clone)]
struct ElemRef<'tx> {
    page: Option<&'tx Page>,      // Option for handling potential nil pages
    node: Option<&'tx Node<'tx>>, // Option for handling potential nil nodes
    index: u32,
}

impl<'tx> ElemRef<'tx> {
    /// isLeaf returns whether the ref is pointing at a leaf page/node.
    fn is_leaf(&self) -> bool {
        // if self.node.is_some() {
        //     let is_leaf = self.node.map_or(false, |n| n.is_leaf());
        //     return is_leaf;
        // }

        // //assert is page
        // if self.page.is_none() {
        //     panic!("ElemRef not page")
        // }

        // self.page.unwrap().is_leaf_page();

        if let Some(node) = self.node {
            return node.is_leaf();
        }

        // assert is page
        if let Some(page) = self.page {
            return page.is_leaf_page();
        }

        panic!("ElemRef not page or node");
    }

    /// count returns the number of inodes or page elements.
    fn count(&self) -> u32 {
        // if self.node.is_some() {
        //     let count = self.node.map_or(0, |n| n.0.inodes.borrow().len()); // Use map_or for optional counting
        //     return count as u32;
        // }
        // //assert is page
        // if self.page.is_none() {
        //     panic!("ElemRef not page")
        // }

        // return self.page.unwrap().count() as u32;

        if let Some(node) = self.node {
            let len = node.0.inodes.borrow().len();
            return len as u32;
        }

        if let Some(page) = self.page {
            return page.count() as u32;
        }

        panic!("ElemRef not page or node");
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
