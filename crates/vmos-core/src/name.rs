//! Identifier validation shared by the CLI, daemon and SDK.
//!
//! All identifiers become path components, so they are restricted to a
//! conservative character set and can never be `.`, `..`, or hidden files.

use crate::error::{CoreError, Result};

const MAX_NAME: usize = 64;

fn bad(value: &str, reason: &'static str) -> CoreError {
    CoreError::InvalidName {
        name: value.to_string(),
        reason,
    }
}

fn bad_id(value: &str, reason: &'static str) -> CoreError {
    CoreError::InvalidId {
        value: value.to_string(),
        reason,
    }
}

pub fn validate_vm_name(value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(bad(value, "must not be empty"));
    }
    if value.len() > MAX_NAME {
        return Err(bad(value, "must be at most 64 bytes"));
    }
    if value == "." || value == ".." {
        return Err(bad(value, "must not be '.' or '..'"));
    }
    if value.starts_with('.') || value.starts_with('-') {
        return Err(bad(value, "must not start with '.' or '-'"));
    }
    if value.ends_with('.') || value.ends_with(' ') {
        return Err(bad(value, "must not end with '.' or a space"));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(bad(
            value,
            "only ASCII letters, digits, '-', '_' and '.' are allowed",
        ));
    }
    if value.eq_ignore_ascii_case("daemon") {
        return Err(bad(value, "'daemon' is reserved"));
    }
    Ok(())
}

pub fn validate_snapshot_name(value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(CoreError::InvalidId {
            value: value.to_string(),
            reason: "snapshot name must not be empty",
        });
    }
    if value.len() > MAX_NAME {
        return Err(bad_id(value, "snapshot name must be at most 64 bytes"));
    }
    if value == "." || value == ".." || value.starts_with('.') || value.starts_with('-') {
        return Err(bad_id(value, "invalid snapshot name"));
    }
    if value.ends_with('.') || value.ends_with(' ') {
        return Err(bad_id(
            value,
            "snapshot name must not end with '.' or a space",
        ));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(bad_id(
            value,
            "only ASCII letters, digits, '-', '_' and '.' are allowed",
        ));
    }
    Ok(())
}

pub fn validate_disk_id(value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(bad_id(value, "disk id must not be empty"));
    }
    if value.len() > 32 {
        return Err(bad_id(value, "disk id must be at most 32 bytes"));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(bad_id(
            value,
            "disk id may only contain ASCII letters, digits, '-' and '_'",
        ));
    }
    Ok(())
}

pub fn validate_share_tag(value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(bad_id(value, "shared directory tag must not be empty"));
    }
    if value.len() > 64 {
        return Err(bad_id(
            value,
            "shared directory tag must be at most 64 bytes",
        ));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(bad_id(
            value,
            "shared directory tag may only contain ASCII letters, digits, '-', '_' and '.'",
        ));
    }
    Ok(())
}

/// Validate that a path is relative and cannot escape its base directory.
pub fn validate_relative_path(value: &str) -> Result<()> {
    use std::path::Path;

    let path = Path::new(value);
    if path.is_absolute() {
        return Err(bad_id(value, "path must be relative to the VM directory"));
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(bad_id(value, "path must not contain '..'"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_names() {
        for name in ["dev", "ubuntu-24.04", "my_vm.1", "A1"] {
            validate_vm_name(name).unwrap();
        }
    }

    #[test]
    fn rejects_unsafe_names() {
        for name in ["", ".", "..", ".hidden", "-flag", "a/b", "a b", "café"] {
            assert!(
                validate_vm_name(name).is_err(),
                "{name:?} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_overlong_names() {
        assert!(validate_vm_name(&"a".repeat(65)).is_err());
        assert!(validate_vm_name(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn relative_paths_stay_inside() {
        validate_relative_path("disks/root.img").unwrap();
        assert!(validate_relative_path("../escape.img").is_err());
        assert!(validate_relative_path("/etc/passwd").is_err());
    }
}
