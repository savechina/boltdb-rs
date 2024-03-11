use std::sync::{Arc, Weak};

pub(crate) struct RawDB {}

#[derive(Clone)]
pub struct DB(pub(crate) Arc<RawDB>);

#[derive(Clone, Debug)]
pub(crate) struct WeakDB(Weak<RawDB>);

impl WeakDB {
    pub(crate) fn new() -> WeakDB {
        WeakDB(Weak::new())
    }

    pub(crate) fn upgrade(&self) -> Option<DB> {
        self.0.upgrade().map(DB)
    }

    pub(crate) fn from(db: &DB) -> WeakDB {
        WeakDB(Arc::downgrade(&db.0))
    }
}
