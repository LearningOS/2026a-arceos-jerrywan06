//! A filesystem with file contents and directory entries stored in memory.

#[path = "dir.rs"]
mod dir;
#[path = "file.rs"]
mod file;

pub use self::dir::DirNode;
pub use self::file::FileNode;

use super::RamFsMutex;
use alloc::sync::Arc;
use axfs_vfs::{VfsNodeRef, VfsOps, VfsResult};

/// A RAM filesystem that implements [`axfs_vfs::VfsOps`].
pub struct RamFileSystem {
    parent: RamFsMutex<Option<VfsNodeRef>>,
    root: Arc<DirNode>,
}

impl RamFileSystem {
    /// Creates a new RAM filesystem.
    pub fn new() -> Self {
        Self {
            parent: RamFsMutex::new(None),
            root: DirNode::new(None),
        }
    }

    /// Returns the root directory node.
    pub fn root_dir_node(&self) -> Arc<DirNode> {
        self.root.clone()
    }
}

impl VfsOps for RamFileSystem {
    fn mount(&self, _path: &str, mount_point: VfsNodeRef) -> VfsResult {
        if let Some(parent) = mount_point.parent() {
            self.root
                .set_parent(Some(self.parent.lock().get_or_insert(parent)));
        } else {
            self.root.set_parent(None);
        }
        Ok(())
    }

    fn root_dir(&self) -> VfsNodeRef {
        self.root.clone()
    }
}

impl Default for RamFileSystem {
    fn default() -> Self {
        Self::new()
    }
}
