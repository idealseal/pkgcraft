use std::ffi::{c_char, c_int};
use std::slice;
use std::sync::Arc;

use pkgcraft::repo::fake::Repo as FakeRepo;
use pkgcraft::repo::Repo;

use crate::error::Error;
use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// Create a fake repo from an array of CPV strings.
///
/// Returns NULL on error.
///
/// # Safety
/// The cpvs argument should be valid CPV strings.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_fake_new(
    id: *const c_char,
    priority: c_int,
    cpvs: *mut *mut c_char,
    len: usize,
) -> *mut Repo {
    ffi_catch_panic! {
        let id = try_str_from_ptr!(id);
        let mut cpv_strs = vec![];
        for ptr in unsafe { slice::from_raw_parts(cpvs, len) } {
            let s = try_str_from_ptr!(*ptr);
            cpv_strs.push(s);
        }
        let repo = FakeRepo::new(id, priority).pkgs(cpv_strs);
        Box::into_raw(Box::new(repo.into()))
    }
}

/// Add pkgs to an existing fake repo from an array of CPV strings.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be a non-null Repo pointer and an array of CPV strings.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_fake_extend(
    r: *mut Repo,
    cpvs: *mut *mut c_char,
    len: usize,
) -> *mut Repo {
    ffi_catch_panic! {
        let repo = try_mut_from_ptr!(r);
        let repo = repo.as_fake_mut().expect("invalid repo type: {repo:?}");
        let repo = unwrap_or_panic!(
            Arc::get_mut(repo).ok_or_else(|| Error::new("failed getting mutable repo ref"))
        );

        let mut cpv_strs = vec![];
        for s in unsafe { slice::from_raw_parts(cpvs, len) } {
            let s = try_str_from_ptr!(*s);
            cpv_strs.push(s);
        }

        repo.extend(cpv_strs);
        r
    }
}
