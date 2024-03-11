//!
//!

extern crate bitflags;
extern crate fnv;
extern crate page_size;

mod bucket;
mod common;
mod errors;
mod node;
mod os;
pub mod tx;
pub mod db;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        println!("{}", errors::BoltError::Checksum);

        let pid: common::page::PgId = 64;
        assert_eq!(2 + 2, 4);
    }
}
