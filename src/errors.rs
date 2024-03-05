//! Package errors defines the error variables that may be returned
//!  during bbolt operations.

use std::io;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum BoltError {
    /// ErrConfig
    #[error("invalid Configuration: {0}")]
    Config(String),

    /// Returned when io be opened failed.
    #[error("io error: {0}")]
    Io(String),
    /// Returned when file be resized failed.
    #[error("resize failed")]
    ResizeFail,
    #[error("tx managed")]
    TxManaged,
    #[error("stack empty")]
    StackEmpty,
    /// Returned when check sync failed.
    #[error("check failed, {0}")]
    CheckFailed(String),

    /// ErrUnexpected is returned when  Unexpected operation
    #[error("{0}")]
    Unexpected(&'static str),

    ///////////////////////////////////////////////////////////////////////////
    // These errors can be returned when opening or calling methods on a DB.
    ///////////////////////////////////////////////////////////////////////////
    /// ErrDatabaseNotOpen is returned when a DB instance is accessed before it
    /// is opened or after it is closed.
    #[error("database not open")]
    DatabaseNotOpen,

    /// ErrInvalid is returned when both meta pages on a database are invalid.
    /// This typically occurs when a file is not a bolt database.
    #[error("invalid database")]
    Invalid,

    /// ErrInvalidMapping is returned when the database file fails to get mapped.
    #[error("database isn't correctly mapped")]
    InvalidMapping,

    /// ErrVersionMismatch is returned when the data file was created with a
    /// different version of Bolt.
    #[error("version mismatch")]
    VersionMismatch,

    /// ErrChecksum is returned when either meta page checksum does not match.
    #[error("checksum error")]
    Checksum,

    /// ErrTimeout is returned when a database cannot obtain an exclusive lock
    // on the data file after the timeout passed to Open().
    #[error("timeout")]
    Timeout,

    ///////////////////////////////////////////////////////////////////////////
    // These errors can occur when beginning or committing a Tx.
    ///////////////////////////////////////////////////////////////////////////
    /// ErrTxNotWritable is returned when performing a write operation on a
    /// read-only transaction.
    #[error("tx not writable")]
    TxNotWritable,

    /// ErrTxClosed is returned when committing or rolling back a transaction
    /// that has already been committed or rolled back.
    #[error("tx closed")]
    TxClosed,

    /// ErrDatabaseReadOnly is returned when a mutating transaction is started on a
    /// read-only database.
    #[error("database is in read-only mode")]
    DatabaseReadOnly,

    /// ErrFreePagesNotLoaded is returned when a readonly transaction without
    /// preloading the free pages is trying to access the free pages.
    #[error("free pages are not pre-loaded")]
    FreePagesNotLoaded,

    ///////////////////////////////////////////////////////////////////////////
    // These errors can occur when putting or deleting a value or a bucket.
    ///////////////////////////////////////////////////////////////////////////
    /// ErrBucketNotFound is returned when trying to access a bucket that has
    /// not been created yet.
    #[error("bucket not found")]
    BucketNotFound,

    /// ErrBucketExists is returned when creating a bucket that already exists.
    #[error("bucket already exists")]
    BucketExists,

    /// ErrBucketNameRequired is returned when creating a bucket with a blank name.
    #[error("bucket name required")]
    BucketNameRequired,

    /// ErrKeyRequired is returned when inserting a zero-length key.
    #[error("key required")]
    KeyRequired,

    /// ErrKeyTooLarge is returned when inserting a key that is larger than MaxKeySize.
    #[error("key too large")]
    KeyTooLarge,

    /// ErrValueTooLarge is returned when inserting a value that is larger than MaxValueSize.
    #[error("value too large")]
    ValueTooLarge,

    /// ErrIncompatibleValue is returned when trying to create or delete a bucket
    /// on an existing non-bucket key or when trying to create or delete a
    /// non-bucket key on an existing bucket key.
    #[error("incompatible value")]
    IncompatibleValue,

    /// ErrSameBuckets is returned when trying to move a sub-bucket between
    /// source and target buckets, while source and target buckets are the same.
    #[error("the source and target are the same bucket")]
    SameBuckets,

    /// ErrDifferentDB is returned when trying to move a sub-bucket between
    /// source and target buckets, while source and target buckets are in different database files.
    #[error("the source and target buckets are in different database files")]
    DifferentDB,
}

impl From<io::Error> for BoltError {
    #[inline]
    fn from(e: io::Error) -> Self {
        Self::Io(e.kind().to_string())
    }
}

impl From<&'static str> for BoltError {
    #[inline]
    fn from(s: &'static str) -> Self {
        Self::Unexpected(s)
    }
}

pub type Result<T> = std::result::Result<T, BoltError>;

// pub(crate) fn is_valid_error(err: &std::io::Error) -> bool {
//     err.kind() == Uncategorized && err.to_string() == "Success (os error 0)"
// }
