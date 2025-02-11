# boltdb-rs


boltdb-rs is port of Go boltdb( [etcd-io/bbolt](https://github.com/etcd-io/bbolt)) database in Rust. It is a pure Rust implementation of the BoltDB key-value store. 



## Features

- Pure Rust implementation
- Supports all BoltDB features


## Installation

To install boltdb-rs, you can use Cargo:

    cargo add boltdb-rs
    
## Usage

Here's a simple example of how to use boltdb-rs:

    use boltdb_rs::{self, DB};
    fn main() -> Result<(), Box<dyn std::error::Error>> {
        // Open the database in read-write mode
        let db = DB::open("mydb.db", self::Options::default())?;
        // Insert a key-value pair
        db.put(b"key1", b"value1")?;
        // Retrieve the value associated with the key
        let value = db.get(b"key1")?;
        println!("Value: {:?}", value);
        Ok(())
    }


Note: This is a placeholder for the actual content of boltdb-rs/README.md. You should replace it with the actual content of the file. 