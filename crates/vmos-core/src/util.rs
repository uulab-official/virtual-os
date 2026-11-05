//! Filesystem, size and process helpers.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Create a directory tree and restrict the final directory to the owner.
pub fn create_private_dir_all(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|e| CoreError::io(path, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| CoreError::io(path, e))?;
    }
    Ok(())
}

/// Restrict a file to the owner (0600).
pub fn set_private_file(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| CoreError::io(path, e))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Unique temporary path in the same directory as `path`.
pub fn temp_sibling(path: &Path, tag: &str) -> PathBuf {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    parent.join(format!(".{name}.{tag}.{}.tmp", Uuid::new_v4().simple()))
}

/// Write a file atomically: temp file in the same directory, fsync, rename,
/// then fsync the parent directory.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            create_private_dir_all(parent)?;
        }
    }
    let tmp = temp_sibling(path, "write");
    {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)
            .map_err(|e| CoreError::io(&tmp, e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
        }
        file.write_all(bytes).map_err(|e| CoreError::io(&tmp, e))?;
        file.sync_all().map_err(|e| CoreError::io(&tmp, e))?;
    }
    if let Err(error) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(CoreError::io(path, error));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fsync_dir(parent)?;
        }
    }
    Ok(())
}

pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| CoreError::json(path, e))?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = std::fs::read(path).map_err(|e| CoreError::io(path, e))?;
    serde_json::from_slice(&bytes).map_err(|e| CoreError::json(path, e))
}

pub fn fsync_dir(path: &Path) -> Result<()> {
    let dir = File::open(path).map_err(|e| CoreError::io(path, e))?;
    dir.sync_all().map_err(|e| CoreError::io(path, e))
}

/// Whether a process with `pid` currently exists.
pub fn process_alive(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    #[cfg(unix)]
    {
        // SAFETY: kill(2) with signal 0 only performs an existence check.
        let rc = unsafe { libc::kill(pid as i32, 0) };
        if rc == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Parse human-friendly binary sizes (`512M`, `20GiB`, `1.5T`, `4096`).
pub fn parse_size(value: &str) -> std::result::Result<u64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("size is empty".into());
    }
    let lower = trimmed.to_ascii_lowercase();
    let (number, suffix) = {
        let split = lower
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(lower.len());
        lower.split_at(split)
    };
    if number.is_empty() || number == "." {
        return Err(format!("invalid size {value:?}"));
    }
    let amount: f64 = number
        .parse()
        .map_err(|_| format!("invalid size {value:?}"))?;
    if !amount.is_finite() || amount < 0.0 {
        return Err(format!("invalid size {value:?}"));
    }
    let multiplier: u64 = match suffix.trim() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1 << 10,
        "m" | "mb" | "mib" => 1 << 20,
        "g" | "gb" | "gib" => 1 << 30,
        "t" | "tb" | "tib" => 1u64 << 40,
        "p" | "pb" | "pib" => 1u64 << 50,
        _ => return Err(format!("unknown size suffix in {value:?}")),
    };
    let bytes = amount * multiplier as f64;
    if bytes > u64::MAX as f64 {
        return Err(format!("size {value:?} is too large"));
    }
    Ok(bytes.round() as u64)
}

/// Format bytes using binary units with one decimal place.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Read the last `count` lines of a log without loading the whole file.
pub fn read_last_lines(path: &Path, count: usize) -> Result<Vec<String>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    const MAX_TAIL: u64 = 4 * 1024 * 1024;
    let mut file = File::open(path).map_err(|e| CoreError::io(path, e))?;
    let len = file.metadata().map_err(|e| CoreError::io(path, e))?.len();
    if len == 0 {
        return Ok(Vec::new());
    }
    let read_len = len.min(MAX_TAIL);
    file.seek(SeekFrom::Start(len - read_len))
        .map_err(|e| CoreError::io(path, e))?;
    let mut buffer = Vec::with_capacity(read_len as usize);
    file.read_to_end(&mut buffer)
        .map_err(|e| CoreError::io(path, e))?;
    let text = String::from_utf8_lossy(&buffer);
    let mut lines: Vec<&str> = text.split('\n').collect();
    // Drop the first fragment when the read window started mid-line.
    if read_len < len {
        lines.remove(0);
    }
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let start = lines.len().saturating_sub(count);
    Ok(lines[start..]
        .iter()
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("1024").unwrap(), 1024);
        assert_eq!(parse_size("1M").unwrap(), 1024 * 1024);
        assert_eq!(parse_size("1.5GiB").unwrap(), 3 * 1024 * 1024 * 1024 / 2);
        assert!(parse_size("12X").is_err());
        assert!(parse_size("-1").is_err());
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1024), "1.0 KiB");
        assert_eq!(format_size(20 * 1024 * 1024 * 1024), "20.0 GiB");
    }

    #[test]
    fn atomic_write_replaces_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b.json");
        atomic_write(&path, b"one").unwrap();
        atomic_write(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let entries: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn reads_tail_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("serial.log");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        assert_eq!(read_last_lines(&path, 2).unwrap(), vec!["two", "three"]);
        assert_eq!(
            read_last_lines(&path, 10).unwrap(),
            vec!["one", "two", "three"]
        );
    }
}
