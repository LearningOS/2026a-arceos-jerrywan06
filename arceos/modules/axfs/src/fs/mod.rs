cfg_if::cfg_if! {
    if #[cfg(feature = "myfs")] {
        pub mod myfs;
    } else if #[cfg(feature = "fatfs")] {
        pub mod fatfs;
    }
}

#[cfg(feature = "devfs")]
pub use axfs_devfs as devfs;

#[cfg(any(feature = "ramfs", feature = "procfs", feature = "sysfs", feature = "myfs"))]
use axsync::spin::SpinNoIrq as RamFsMutex;

// Share the in-tree implementation with the standalone RAM filesystem crate.
#[cfg(any(feature = "ramfs", feature = "procfs", feature = "sysfs", feature = "myfs"))]
#[path = "../../../../axfs_ramfs/src/fs.rs"]
pub mod ramfs;
