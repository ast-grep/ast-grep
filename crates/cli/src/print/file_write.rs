use anyhow::Result;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Replace a writable source file only after its content and access controls are ready.
pub(super) fn write_atomically(path: &Path, content: &str) -> Result<()> {
  let path = path.canonicalize()?;
  // Check source write access without truncating it.
  let source = OpenOptions::new().write(true).open(&path)?;
  let parent = path.parent().unwrap_or_else(|| Path::new("."));
  let mut temp = tempfile::NamedTempFile::new_in(parent)?;
  // Apply access controls before writing source content into the temporary file.
  let permissions = preserve_access(&source, temp.as_file())?;
  temp.write_all(content.as_bytes())?;
  // Writing can clear special mode bits, so restore the original mode afterwards.
  temp.as_file().set_permissions(permissions)?;
  // Close the source handle before replacing it, including on Windows.
  drop(source);
  temp.persist(path).map_err(|error| error.error)?;
  Ok(())
}

fn preserve_access(source: &File, replacement: &File) -> Result<std::fs::Permissions> {
  let metadata = source.metadata()?;
  #[cfg(unix)]
  {
    use rustix::fs::{Gid, Uid, fchown};
    use std::os::unix::fs::MetadataExt;

    let current = replacement.metadata()?;
    let owner = (current.uid() != metadata.uid()).then(|| Uid::from_raw(metadata.uid()));
    let group = (current.gid() != metadata.gid()).then(|| Gid::from_raw(metadata.gid()));
    if owner.is_some() || group.is_some() {
      fchown(replacement, owner, group)?;
    }
  }
  let permissions = metadata.permissions();
  replacement.set_permissions(permissions.clone())?;
  #[cfg(unix)]
  preserve_acl(source, replacement)?;
  Ok(permissions)
}

#[cfg(target_os = "macos")]
fn preserve_acl(source: &File, replacement: &File) -> Result<()> {
  use std::ffi::{c_int, c_void};
  use std::os::fd::AsRawFd;

  unsafe extern "C" {
    fn acl_init(count: c_int) -> *mut c_void;
    fn acl_get_fd(fd: c_int) -> *mut c_void;
    fn acl_set_fd(fd: c_int, acl: *mut c_void) -> c_int;
    fn acl_free(acl: *mut c_void) -> c_int;
  }

  // SAFETY: source is an open file; a successful call returns an owned ACL handle.
  let mut acl = unsafe { acl_get_fd(source.as_raw_fd()) };
  if acl.is_null() {
    let error = std::io::Error::last_os_error();
    if error.kind() != std::io::ErrorKind::NotFound {
      return Err(error.into());
    }
    // An empty ACL removes any access inherited by the replacement from its directory.
    // SAFETY: zero is a valid initial entry count; the returned handle is owned.
    acl = unsafe { acl_init(0) };
    if acl.is_null() {
      return Err(std::io::Error::last_os_error().into());
    }
  }
  // SAFETY: replacement is open and acl is valid until freed below.
  let result = unsafe { acl_set_fd(replacement.as_raw_fd(), acl) };
  let error = (result != 0).then(std::io::Error::last_os_error);
  // SAFETY: acl is an owned handle and is freed exactly once, including on failure.
  unsafe { acl_free(acl) };
  if let Some(error) = error {
    return Err(error.into());
  }
  Ok(())
}

#[cfg(target_os = "linux")]
fn preserve_acl(source: &File, replacement: &File) -> Result<()> {
  use rustix::fs::{XattrFlags, fremovexattr, fsetxattr};

  let source_attrs = access_attributes(source)?;
  let replacement_attrs = access_attributes(replacement)?;
  let acl = c"system.posix_acl_access";
  match source_attrs.get(acl) {
    Some(value) => fsetxattr(replacement, acl, value, XattrFlags::empty())?,
    None if replacement_attrs.contains_key(acl) => fremovexattr(replacement, acl)?,
    None => {}
  }
  // Refuse to change other access controls, such as a security label or an NFS ACL.
  anyhow::ensure!(
    source_attrs == access_attributes(replacement)?,
    "Cannot preserve source file access controls"
  );
  Ok(())
}

#[cfg(target_os = "linux")]
fn access_attributes(
  file: &File,
) -> Result<std::collections::BTreeMap<std::ffi::CString, Vec<u8>>> {
  use rustix::fs::{fgetxattr, flistxattr};
  use rustix::io::Errno;
  use std::ffi::CString;

  let mut attributes = std::collections::BTreeMap::new();
  let size = match flistxattr(file, &mut [0u8; 0][..]) {
    Ok(size) => size,
    Err(Errno::NOTSUP) => return Ok(attributes),
    Err(error) => return Err(error.into()),
  };
  if size == 0 {
    return Ok(attributes);
  }
  let mut names = vec![0; size];
  let size = flistxattr(file, names.as_mut_slice())?;
  for name in names[..size].split(|byte| *byte == 0) {
    if name.is_empty() || name.starts_with(b"user.") {
      continue;
    }
    let name = CString::new(name)?;
    let size = fgetxattr(file, &name, &mut [0u8; 0][..])?;
    let mut value = vec![0; size];
    let size = fgetxattr(file, &name, value.as_mut_slice())?;
    value.truncate(size);
    attributes.insert(name, value);
  }
  Ok(attributes)
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
fn preserve_acl(_source: &File, _replacement: &File) -> Result<()> {
  anyhow::bail!("Preserving file access controls is unsupported on this platform")
}
