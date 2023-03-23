use std::ffi::c_char;

use pkgcraft::eapi::{Eapi, IntoEapi};
use pkgcraft::repo::temp::Repo as TempRepo;

use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// Create a temporary ebuild repository.
///
/// Returns NULL on error.
///
/// # Safety
/// The id argument should be a valid, unicode string and the eapi parameter can optionally be
/// NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_new(
    id: *const c_char,
    eapi: *const Eapi,
) -> *mut TempRepo {
    ffi_catch_panic! {
        let id = try_str_from_ptr!(id);
        let eapi = unwrap_or_panic!(IntoEapi::into_eapi(eapi));
        let repo = unwrap_or_panic!(TempRepo::new(id, None, Some(eapi)));
        Box::into_raw(Box::new(repo))
    }
}

/// Return a temporary repo's path.
///
/// # Safety
/// The argument must be a non-null TempRepo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_path(r: *mut TempRepo) -> *mut c_char {
    let repo = try_ref_from_ptr!(r);
    try_ptr_from_str!(repo.path().as_str())
}

/// Persist a temporary repo to disk, returning its path.
///
/// # Safety
/// The related TempRepo pointer is invalid on function completion and should not be used.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_persist(
    r: *mut TempRepo,
    path: *const c_char,
) -> *mut c_char {
    ffi_catch_panic! {
        let repo = unsafe { r.read() };
        let repo_path = match path.is_null() {
            true => None,
            false => Some(try_str_from_ptr!(path)),
        };
        let path = unwrap_or_panic!(repo.persist(repo_path));
        try_ptr_from_str!(path.as_str())
    }
}

/// Free a temporary repo.
///
/// Freeing a temporary repo removes the related directory from the filesystem.
///
/// # Safety
/// The argument must be a TempRepo pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_free(r: *mut TempRepo) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}
