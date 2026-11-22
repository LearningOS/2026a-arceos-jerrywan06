extern crate alloc;

use alloc::sync::Arc;
use axfs_vfs::VfsOps;
use std::os::arceos::api::fs::{AxDisk, MyFileSystemIf, RamFileSystem};

struct MyFileSystemIfImpl;

#[crate_interface::impl_interface]
impl MyFileSystemIf for MyFileSystemIfImpl {
    fn new_myfs(_disk: AxDisk) -> Arc<dyn VfsOps> {
        Arc::new(RamFileSystem::new())
    }
}
