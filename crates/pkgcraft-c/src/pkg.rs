use std::cmp::Ordering;
use std::ffi::{c_char, c_int};

use pkgcraft::dep::Cpv;
use pkgcraft::eapi::Eapi;
use pkgcraft::pkg::{Package, Pkg, RepoPackage};
use pkgcraft::repo::Repo;
use pkgcraft::restrict::{Restrict, Restriction};
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::types::Version;

pub mod ebuild;

#[repr(C)]
pub enum PkgFormat {
    Configured,
    Ebuild,
    Fake,
}

impl From<&Pkg<'_>> for PkgFormat {
    fn from(pkg: &Pkg) -> Self {
        match pkg {
            Pkg::Configured(_, _) => Self::Configured,
            Pkg::Ebuild(_, _) => Self::Ebuild,
            Pkg::Fake(_, _) => Self::Fake,
        }
    }
}

/// Return a package's format.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_format(p: *mut Pkg) -> PkgFormat {
    let pkg = try_ref_from_ptr!(p);
    pkg.into()
}

/// Return a package's CPV.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_cpv(p: *mut Pkg) -> *mut Cpv {
    let pkg = try_ref_from_ptr!(p);
    Box::into_raw(Box::new(pkg.cpv().clone()))
}

/// Return a package's repo.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_repo(p: *mut Pkg) -> *const Repo {
    let pkg = try_ref_from_ptr!(p);
    pkg.repo()
}

/// Return a package's EAPI.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_eapi(p: *mut Pkg) -> *const Eapi {
    let pkg = try_ref_from_ptr!(p);
    pkg.eapi()
}

/// Return a package's version.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_version(p: *mut Pkg) -> *mut Version {
    let pkg = try_ref_from_ptr!(p);
    Box::into_raw(Box::new(pkg.version().clone()))
}

/// Compare two packages returning -1, 0, or 1 if the first package is less than, equal to, or
/// greater than the second package, respectively.
///
/// # Safety
/// The arguments must be non-null Pkg pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_cmp<'a>(p1: *mut Pkg<'a>, p2: *mut Pkg<'a>) -> c_int {
    let pkg1 = try_ref_from_ptr!(p1);
    let pkg2 = try_ref_from_ptr!(p2);

    match pkg1.cmp(pkg2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Return the string for a package.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_str(p: *mut Pkg) -> *mut c_char {
    let pkg = try_ref_from_ptr!(p);
    try_ptr_from_str!(pkg.to_string())
}

/// Return the hash value for a package.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_hash(p: *mut Pkg) -> u64 {
    let pkg = try_ref_from_ptr!(p);
    hash(pkg)
}

/// Return the restriction for a package.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_restrict(p: *mut Pkg) -> *mut Restrict {
    let pkg = try_ref_from_ptr!(p);
    Box::into_raw(Box::new(pkg.into()))
}

/// Determine if a restriction matches a package.
///
/// # Safety
/// The arguments must be valid Restrict and Pkg pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_restrict_matches(p: *mut Pkg, r: *mut Restrict) -> bool {
    let pkg = try_ref_from_ptr!(p);
    let r = try_ref_from_ptr!(r);
    r.matches(pkg)
}

/// Free an package.
///
/// # Safety
/// The argument must be a non-null Pkg pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_free(p: *mut Pkg) {
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p)) };
    }
}
