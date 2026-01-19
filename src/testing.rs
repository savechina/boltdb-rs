use std::ops::{Deref, DerefMut};

use tempfile::{Builder, NamedTempFile};

use crate::{DB, db::Options};

pub(crate) fn temp_file() -> crate::Result<NamedTempFile> {
    let temp_file = Builder::new()
        .prefix("boltdb-rs-")
        .suffix(".db")
        .tempfile()?;

    Ok(temp_file)
}

pub(crate) struct TestDb {
    pub(crate) temp_file: Option<NamedTempFile>,
    pub(crate) db: Option<DB>,
    options: Options,
}

impl Deref for TestDb {
    type Target = DB;

    fn deref(&self) -> &Self::Target {
        self.db.as_ref().unwrap()
    }
}

impl DerefMut for TestDb {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.db.as_mut().unwrap()
    }
}

impl TestDb {
    pub(crate) fn new() -> crate::Result<Self> {
        Self::with_options(Options::default())
    }
    pub(crate) fn with_options(options: Options) -> crate::Result<Self> {
        let temp_file = temp_file()?;

        let db = DB::open_with(temp_file.path(), options.clone())?;

        Ok(Self {
            temp_file: Some(temp_file),
            db: Some(db),
            options,
        })
    }

    pub(crate) fn clone_db(&self) -> DB {
        self.db.as_ref().unwrap().clone()
    }
}
