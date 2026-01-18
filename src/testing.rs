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
    pub(crate) tmp_file: Option<NamedTempFile>,
    pub(crate) db: Option<DB>,
    options: Options,
}

impl TestDb {
    pub(crate) fn new(options: Options) -> crate::Result<Self> {
        let tmp_file = temp_file()?;

        let db = DB::open_with(tmp_file.path(), options.clone())?;

        Ok(Self {
            tmp_file: Some(tmp_file),
            db: Some(db),
            options,
        })
    }
}
