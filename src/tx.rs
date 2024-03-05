pub struct Tx {}
// Tx represents a read-only or read/write transaction on the database.
// Read-only transactions can be used for retrieving values for keys and creating cursors.
// Read/write transactions can create and remove buckets and create and remove keys.
//
// IMPORTANT: You must commit or rollback transactions when you are done with
// them. Pages can not be reclaimed by the writer until no more transactions
// are using them. A long running read transaction can cause the database to
// quickly grow.
// type Tx struct {
// 	writable       bool
// 	managed        bool
// 	db             *DB
// 	meta           *meta
// 	root           Bucket
// 	pages          map[pgid]*page
// 	stats          TxStats
// 	commitHandlers []func()

// WriteFlag specifies the flag for write-related methods like WriteTo().
// Tx opens the database file with the specified flag to copy the data.
//
// By default, the flag is unset, which works well for mostly in-memory
// workloads. For databases that are much larger than available RAM,
// set the flag to syscall.O_DIRECT to avoid trashing the page cache.
// 	WriteFlag int
// }
