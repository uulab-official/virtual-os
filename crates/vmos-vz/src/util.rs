use std::path::Path;

use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_foundation::{NSError, NSString, NSURL};

use crate::error::VzError;

pub(crate) fn path_to_nsurl(path: &Path) -> Result<Retained<NSURL>, VzError> {
    let value = path.to_str().ok_or_else(|| {
        VzError::invalid_config(format!("path is not valid UTF-8: {}", path.display()))
    })?;
    let ns_path = NSString::from_str(value);
    Ok(NSURL::initFileURLWithPath(NSURL::alloc(), &ns_path))
}

/// Render an NSError with its domain, code, reason and cause chain.
pub(crate) fn describe_ns_error(error: &NSError) -> String {
    let mut out = error.localizedDescription().to_string();
    if let Some(reason) = error.localizedFailureReason() {
        out.push_str(" (reason: ");
        out.push_str(&reason.to_string());
        out.push(')');
    }
    out.push_str(&format!(" [{}:{}]", error.domain(), error.code()));
    let underlying = error.underlyingErrors();
    for cause in underlying.iter() {
        out.push_str("\n  caused by: ");
        out.push_str(&describe_ns_error(&cause));
    }
    out
}

pub(crate) fn read_blob(path: &Path) -> Result<Vec<u8>, VzError> {
    std::fs::read(path).map_err(|e| VzError::io(path, e))
}

/// Persist an opaque platform blob atomically.
pub(crate) fn write_blob(path: &Path, bytes: &[u8]) -> Result<(), VzError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| VzError::io(parent, e))?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| VzError::io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| VzError::io(path, e))?;
    Ok(())
}

/// Convert a Rust string into an NSString.
pub(crate) fn nsstring(value: &str) -> Retained<NSString> {
    NSString::from_str(value)
}
