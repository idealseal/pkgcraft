use std::fs;
use std::ops::Deref;
use std::path::Path;
use std::str::FromStr;

use camino::{Utf8DirEntry, Utf8Path};
use itertools::Itertools;
use nix::{sys::stat, unistd};
use walkdir::{DirEntry, WalkDir};

use crate::Error;

#[derive(Debug, Clone)]
pub(crate) struct Group(unistd::Group);

impl FromStr for Group {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match unistd::Group::from_name(s) {
            Ok(Some(val)) => Ok(Group(val)),
            Ok(None) => Err(Error::InvalidValue(format!("unknown group: {s}"))),
            Err(_) => Err(Error::InvalidValue(format!("invalid group: {s}"))),
        }
    }
}

impl Deref for Group {
    type Target = unistd::Group;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct User(unistd::User);

impl Deref for User {
    type Target = unistd::User;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for User {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match unistd::User::from_name(s) {
            Ok(Some(val)) => Ok(User(val)),
            Ok(None) => Err(Error::InvalidValue(format!("unknown user: {s}"))),
            Err(_) => Err(Error::InvalidValue(format!("invalid user: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Mode(stat::Mode);

impl Deref for Mode {
    type Target = stat::Mode;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for Mode {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let without_prefix = s.trim_start_matches("0o");
        let mode = stat::mode_t::from_str_radix(without_prefix, 8)
            .map_err(|_| Error::InvalidValue(format!("invalid mode: {s}")))?;
        let mode = stat::Mode::from_bits(mode)
            .ok_or_else(|| Error::InvalidValue(format!("invalid mode: {s}")))?;
        Ok(Mode(mode))
    }
}

// None value coerced to a directory filtering predicate function pointer for use with
// Option-wrapped closure parameter generics.
type WalkDirFilter = fn(&DirEntry) -> bool;
pub(crate) const NO_WALKDIR_FILTER: Option<WalkDirFilter> = None;

pub(crate) fn sorted_dir_list<P: AsRef<Path>>(path: P) -> WalkDir {
    WalkDir::new(path.as_ref())
        .sort_by_file_name()
        .min_depth(1)
        .max_depth(1)
}

pub(crate) fn is_file(entry: &DirEntry) -> bool {
    entry.path().is_file()
}

pub(crate) fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

pub(crate) fn sorted_dir_list_utf8(path: &Utf8Path) -> crate::Result<Vec<Utf8DirEntry>> {
    let mut entries: Vec<_> = path
        .read_dir_utf8()
        .map_err(|e| Error::IO(format!("failed reading dir: {path}: {e}")))?
        .try_collect()?;
    entries.sort_by(|a, b| a.file_name().cmp(b.file_name()));
    Ok(entries)
}

pub(crate) fn is_dir_utf8(entry: &Utf8DirEntry) -> bool {
    entry.path().is_dir()
}

pub(crate) fn is_file_utf8(entry: &Utf8DirEntry) -> bool {
    entry.path().is_file()
}

pub(crate) fn is_hidden_utf8(entry: &Utf8DirEntry) -> bool {
    entry.file_name().starts_with('.')
}

pub(crate) fn has_ext_utf8(entry: &Utf8DirEntry, ext: &str) -> bool {
    entry
        .path()
        .extension()
        .map(|s| s == ext)
        .unwrap_or_default()
}

/// Create a file atomically by writing to a temporary path and then renaming it.
pub(crate) fn atomic_write_file<C: AsRef<[u8]>>(
    path: &Utf8Path,
    file_name: &str,
    data: C,
) -> crate::Result<()> {
    // create parent dir
    fs::create_dir_all(path)
        .map_err(|e| Error::IO(format!("failed creating metadata dir: {path}: {e}")))?;

    // TODO: support custom temporary file path formats
    let tmp_path = path.join(format!(".{file_name}"));
    let new_path = path.join(file_name);

    // write file to temp path
    fs::write(&tmp_path, data)
        .map_err(|e| Error::IO(format!("failed writing data: {tmp_path}: {e}")))?;

    // move file to final path
    fs::rename(&tmp_path, &new_path)
        .map_err(|e| Error::IO(format!("failed renaming file: {tmp_path} -> {new_path}: {e}")))?;

    Ok(())
}
