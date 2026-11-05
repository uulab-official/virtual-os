//! Sparse raw disk images.
//!
//! Raw images are used instead of a container format so that the on-disk
//! representation stays inspectable and can be attached directly by
//! Virtualization.framework. Snapshots clone images copy-on-write via APFS
//! `clonefile(2)` when available, falling back to a sparse block copy.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::util;

const COPY_CHUNK: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DiskFormat {
    #[default]
    Raw,
}

impl DiskFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Raw => "raw",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskInfo {
    pub id: String,
    pub path: PathBuf,
    pub format: DiskFormat,
    pub size_bytes: u64,
    pub allocated_bytes: u64,
    pub read_only: bool,
    /// False when the backing file is missing.
    pub present: bool,
}

/// Create a sparse image that does not yet exist.
pub fn create_sparse(path: &Path, size_bytes: u64) -> Result<()> {
    if size_bytes == 0 {
        return Err(CoreError::invalid_config(
            "disk size must be greater than zero",
        ));
    }
    if path.exists() {
        return Err(CoreError::Other(format!(
            "refusing to overwrite existing disk image {}",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        util::create_private_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .read(true)
        .open(path)
        .map_err(|e| CoreError::io(path, e))?;
    file.set_len(size_bytes)
        .map_err(|e| CoreError::io(path, e))?;
    file.sync_all().map_err(|e| CoreError::io(path, e))?;
    if let Some(parent) = path.parent() {
        util::fsync_dir(parent)?;
    }
    Ok(())
}

/// Grow an image in place. Shrinking is refused because it can silently
/// discard guest data.
pub fn resize(path: &Path, new_size: u64) -> Result<()> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| CoreError::io(path, e))?;
    let current = file.metadata().map_err(|e| CoreError::io(path, e))?.len();
    if new_size < current {
        return Err(CoreError::Other(format!(
            "refusing to shrink disk from {} to {}",
            util::format_size(current),
            util::format_size(new_size)
        )));
    }
    if new_size == current {
        return Ok(());
    }
    file.set_len(new_size).map_err(|e| CoreError::io(path, e))?;
    file.sync_all().map_err(|e| CoreError::io(path, e))?;
    Ok(())
}

/// Clone `src` to `dst`.
///
/// On APFS this is a metadata-only copy-on-write clone. Other filesystems get
/// a sparse copy that does not write zero blocks.
pub fn clone_file(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        std::fs::remove_file(dst).map_err(|e| CoreError::io(dst, e))?;
    }
    if let Some(parent) = dst.parent() {
        util::create_private_dir_all(parent)?;
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let src_c = CString::new(src.as_os_str().as_bytes())
            .map_err(|_| CoreError::Other("source path contains NUL".into()))?;
        let dst_c = CString::new(dst.as_os_str().as_bytes())
            .map_err(|_| CoreError::Other("destination path contains NUL".into()))?;
        // SAFETY: both CStrings are valid, NUL-terminated paths and the flags
        // value 0 is documented for clonefile(2).
        let rc = unsafe { libc::clonefile(src_c.as_ptr(), dst_c.as_ptr(), 0) };
        if rc == 0 {
            if let Some(parent) = dst.parent() {
                util::fsync_dir(parent)?;
            }
            return Ok(());
        }
        let err = std::io::Error::last_os_error();
        // EXDEV/ENOTSUP/ENOSYS: fall back to a sparse copy. Anything else is
        // a real error, but a best-effort copy is still attempted so that
        // restricted filesystems keep working.
        tracing::debug!(error = %err, "clonefile fallback to sparse copy");
    }

    sparse_copy(src, dst)?;
    if let Some(parent) = dst.parent() {
        util::fsync_dir(parent)?;
    }
    Ok(())
}

/// Copy a file while preserving holes.
pub fn sparse_copy(src: &Path, dst: &Path) -> Result<()> {
    let mut input = File::open(src).map_err(|e| CoreError::io(src, e))?;
    let mut output = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(dst)
        .map_err(|e| CoreError::io(dst, e))?;

    let len = input.metadata().map_err(|e| CoreError::io(src, e))?.len();
    let mut offset = 0u64;
    let mut buffer = vec![0u8; COPY_CHUNK];
    while offset < len {
        let want = std::cmp::min(COPY_CHUNK as u64, len - offset) as usize;
        input
            .read_exact(&mut buffer[..want])
            .map_err(|e| CoreError::io(src, e))?;
        let all_zero = buffer[..want].iter().all(|&b| b == 0);
        if all_zero {
            output
                .seek(SeekFrom::Current(want as i64))
                .map_err(|e| CoreError::io(dst, e))?;
        } else {
            output
                .write_all(&buffer[..want])
                .map_err(|e| CoreError::io(dst, e))?;
        }
        offset += want as u64;
    }
    output.set_len(len).map_err(|e| CoreError::io(dst, e))?;
    output.sync_all().map_err(|e| CoreError::io(dst, e))?;
    Ok(())
}

/// Clone through a temporary sibling and atomically rename over `dst`.
///
/// Used by snapshot restore so a crash cannot leave a half-written disk.
pub fn replace_with_clone(src: &Path, dst: &Path) -> Result<()> {
    let parent = dst
        .parent()
        .ok_or_else(|| CoreError::Other(format!("disk path has no parent: {}", dst.display())))?;
    util::create_private_dir_all(parent)?;
    let tmp = util::temp_sibling(dst, "restore");
    clone_file(src, &tmp)?;
    std::fs::rename(&tmp, dst).map_err(|e| CoreError::io(dst, e))?;
    util::fsync_dir(parent)?;
    Ok(())
}

/// Number of bytes actually allocated on disk.
pub fn allocated_bytes(path: &Path) -> Result<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::metadata(path).map_err(|e| CoreError::io(path, e))?;
        Ok(metadata.blocks().saturating_mul(512))
    }
    #[cfg(not(unix))]
    {
        let metadata = std::fs::metadata(path).map_err(|e| CoreError::io(path, e))?;
        Ok(metadata.len())
    }
}

pub fn inspect(id: &str, path: &Path, read_only: bool, declared_size: u64) -> Result<DiskInfo> {
    let metadata = std::fs::metadata(path).map_err(|e| CoreError::io(path, e))?;
    Ok(DiskInfo {
        id: id.to_string(),
        path: path.to_path_buf(),
        format: DiskFormat::Raw,
        size_bytes: metadata.len().max(declared_size),
        allocated_bytes: allocated_bytes(path)?,
        read_only,
        present: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_sparse_images() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("root.img");
        create_sparse(&path, 64 * 1024 * 1024).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 64 * 1024 * 1024);
        assert!(allocated_bytes(&path).unwrap() < 8 * 1024 * 1024);
        assert!(create_sparse(&path, 1024).is_err());
    }

    #[test]
    fn clone_preserves_contents() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.img");
        let dst = dir.path().join("dst.img");
        create_sparse(&src, 8 * 1024 * 1024).unwrap();
        let mut file = OpenOptions::new().write(true).open(&src).unwrap();
        file.seek(SeekFrom::Start(4 * 1024 * 1024)).unwrap();
        file.write_all(b"vmos").unwrap();
        drop(file);
        clone_file(&src, &dst).unwrap();
        assert_eq!(std::fs::read(&dst).unwrap().len(), 8 * 1024 * 1024);
        assert_eq!(
            &std::fs::read(&dst).unwrap()[4 * 1024 * 1024..4 * 1024 * 1024 + 4],
            b"vmos"
        );
    }

    #[test]
    fn resize_only_grows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("root.img");
        create_sparse(&path, 4 * 1024 * 1024).unwrap();
        resize(&path, 8 * 1024 * 1024).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 8 * 1024 * 1024);
        assert!(resize(&path, 1024).is_err());
    }
}
