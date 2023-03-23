use std::cmp::Ordering;
use std::ffi::{c_char, c_int};
use std::{ptr, slice};

use pkgcraft::pkg::Pkg;
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::{PkgRepository, Repo};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::types::RepoSetIter;
use crate::utils::str_to_raw;

#[repr(C)]
pub enum RepoSetOp {
    And,
    Or,
    Xor,
    Sub,
}

/// Create a repo set.
///
/// # Safety
/// The argument must be an array of Repo pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_new(repos: *mut *mut Repo, len: usize) -> *mut RepoSet {
    let repos = unsafe { slice::from_raw_parts(repos, len) };
    let repos = repos.iter().map(|r| try_ref_from_ptr!(r));
    Box::into_raw(Box::new(RepoSet::new(repos)))
}

/// Return a repo set's categories.
///
/// # Safety
/// The argument must be a non-null RepoSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_categories(
    s: *mut RepoSet,
    len: *mut usize,
) -> *mut *mut c_char {
    let s = try_ref_from_ptr!(s);
    iter_to_array!(s.categories().iter(), len, str_to_raw)
}

/// Return a repo set's packages for a category.
///
/// # Safety
/// The arguments must be a non-null RepoSet pointer and category.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_packages(
    s: *mut RepoSet,
    cat: *const c_char,
    len: *mut usize,
) -> *mut *mut c_char {
    let s = try_ref_from_ptr!(s);
    let cat = try_str_from_ptr!(cat);
    iter_to_array!(s.packages(cat).iter(), len, str_to_raw)
}

/// Return a repo set's versions for a package.
///
/// # Safety
/// The arguments must be a non-null RepoSet pointer, category, and package.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_versions(
    s: *mut RepoSet,
    cat: *const c_char,
    pkg: *const c_char,
    len: *mut usize,
) -> *mut *mut c_char {
    let s = try_ref_from_ptr!(s);
    let cat = try_str_from_ptr!(cat);
    let pkg = try_str_from_ptr!(pkg);
    iter_to_array!(s.versions(cat, pkg).iter(), len, str_to_raw)
}

/// Return a repo set's length.
///
/// # Safety
/// The argument must be a non-null RepoSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_len(s: *mut RepoSet) -> usize {
    let s = try_ref_from_ptr!(s);
    s.len()
}

/// Determine if a repo set is empty.
///
/// # Safety
/// The argument must be a non-null RepoSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_is_empty(s: *mut RepoSet) -> bool {
    let s = try_ref_from_ptr!(s);
    s.is_empty()
}

/// Return the ordered array of repos for a repo set.
///
/// # Safety
/// The argument must be a non-null RepoSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_repos(
    s: *mut RepoSet,
    len: *mut usize,
) -> *mut *const Repo {
    let s = try_ref_from_ptr!(s);
    iter_to_array!(s.repos().iter(), len, |r| { r as *const _ })
}

/// Compare two repo sets returning -1, 0, or 1 if the first set is less than, equal to, or greater
/// than the second set, respectively.
///
/// # Safety
/// The arguments must be non-null RepoSet pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_cmp(s1: *mut RepoSet, s2: *mut RepoSet) -> c_int {
    let s1 = try_ref_from_ptr!(s1);
    let s2 = try_ref_from_ptr!(s2);

    match s1.cmp(s2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Return the hash value for a repo set.
///
/// # Safety
/// The argument must be a non-null RepoSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_hash(s: *mut RepoSet) -> u64 {
    let s = try_ref_from_ptr!(s);
    hash(s)
}

/// Free a repo set.
///
/// # Safety
/// The argument must be a RepoSet pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_free(r: *mut RepoSet) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}

/// Return a package iterator for a repo set.
///
/// # Safety
/// The repo argument must be a non-null Repo pointer and the restrict argument can be a
/// Restrict pointer or NULL to iterate over all packages.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_iter<'a>(
    s: *mut RepoSet,
    restrict: *mut Restrict,
) -> *mut RepoSetIter<'a> {
    let s = try_ref_from_ptr!(s);
    let iter = match unsafe { restrict.as_ref() } {
        Some(r) => s.iter_restrict(r.clone()),
        None => s.iter(),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next package from a repo set package iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null RepoSetIter pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_iter_next(i: *mut RepoSetIter) -> *mut Pkg {
    let iter = try_mut_from_ptr!(i);
    match iter.next() {
        Some(p) => Box::into_raw(Box::new(p)),
        None => ptr::null_mut(),
    }
}

/// Free a repo set iterator.
///
/// # Safety
/// The argument must be a non-null RepoSetIter pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_iter_free(i: *mut RepoSetIter) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Perform a set operation on two repo sets, assigning to the first set.
///
/// # Safety
/// The arguments must be non-null RepoSet pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_assign_op_set(
    op: RepoSetOp,
    s1: *mut RepoSet,
    s2: *mut RepoSet,
) {
    use RepoSetOp::*;
    let s1 = try_mut_from_ptr!(s1);
    let s2 = try_ref_from_ptr!(s2);
    match op {
        And => *s1 &= s2,
        Or => *s1 |= s2,
        Xor => *s1 ^= s2,
        Sub => *s1 -= s2,
    }
}

/// Perform a set operation on a repo set and repo, assigning to the set.
///
/// # Safety
/// The arguments must be non-null RepoSet and Repo pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_assign_op_repo(
    op: RepoSetOp,
    s: *mut RepoSet,
    r: *mut Repo,
) {
    use RepoSetOp::*;
    let s = try_mut_from_ptr!(s);
    let r = try_ref_from_ptr!(r);
    match op {
        And => *s &= r,
        Or => *s |= r,
        Xor => *s ^= r,
        Sub => *s -= r,
    }
}

/// Perform a set operation on two repo sets, creating a new set.
///
/// # Safety
/// The arguments must be non-null RepoSet pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_op_set(
    op: RepoSetOp,
    s1: *mut RepoSet,
    s2: *mut RepoSet,
) -> *mut RepoSet {
    use RepoSetOp::*;
    let s1 = try_mut_from_ptr!(s1);
    let s2 = try_ref_from_ptr!(s2);
    let set = match op {
        And => s1.clone() & s2,
        Or => s1.clone() | s2,
        Xor => s1.clone() ^ s2,
        Sub => s1.clone() - s2,
    };
    Box::into_raw(Box::new(set))
}

/// Perform a set operation on a repo set and repo, creating a new set.
///
/// # Safety
/// The arguments must be non-null RepoSet and Repo pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_set_op_repo(
    op: RepoSetOp,
    s: *mut RepoSet,
    r: *mut Repo,
) -> *mut RepoSet {
    use RepoSetOp::*;
    let s = try_mut_from_ptr!(s);
    let r = try_ref_from_ptr!(r);
    let set = match op {
        And => s.clone() & r,
        Or => s.clone() | r,
        Xor => s.clone() ^ r,
        Sub => s.clone() - r,
    };
    Box::into_raw(Box::new(set))
}
