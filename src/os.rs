//!
//! 
//!

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "mips64",
    target_arch= "loongarch64"
))]
// maxMapSize represents the largest mmap size supported by Bolt.
pub const MAX_MAP_SIZE: u64 = 0xFFFFFFFFFFFF; // 256TB

#[cfg(any(
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "mips",
    target_arch = "powerpc"
))]
// maxMapSize represents the largest mmap size supported by Bolt.
pub const MAX_MAP_SIZE :u64= 0x7FFFFFFF; // 2GB

// maxAllocSize is the size used when creating array pointers.
pub const MAX_ALLOC_SIZE :u64= 0x7FFFFFFF;