use std::ffi::{c_char, c_int};

use pkgcraft::config::{Config, RepoSetType};
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::Repo;

use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// Create an empty pkgcraft config.
#[no_mangle]
pub extern "C" fn pkgcraft_config_new() -> *mut Config {
    let config = Config::new("pkgcraft", "");
    Box::into_raw(Box::new(config))
}

/// Add local repo from filesystem path.
///
/// Returns NULL on error.
///
/// # Safety
/// The path argument should be a valid path on the system.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_add_repo_path(
    c: *mut Config,
    id: *const c_char,
    priority: c_int,
    path: *const c_char,
) -> *mut Repo {
    ffi_catch_panic! {
        let path = try_str_from_ptr!(path);
        let id = match id.is_null() {
            true => path,
            false => try_str_from_ptr!(id),
        };

        let config = try_mut_from_ptr!(c);
        let repo = unwrap_or_panic!(config.add_repo_path(id, priority, path));
        Box::into_raw(Box::new(repo))
    }
}

/// Add an external Repo to the config.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be valid Config and Repo pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_add_repo(c: *mut Config, r: *mut Repo) -> *mut Repo {
    ffi_catch_panic! {
        let config = try_mut_from_ptr!(c);
        let repo = try_ref_from_ptr!(r);
        unwrap_or_panic!(config.add_repo(repo));
        r
    }
}

/// Load the system config.
///
/// Returns NULL on error.
///
/// # Safety
/// A valid pkgcraft (or portage config) directory should be located on the system.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_load(c: *mut Config) -> *mut Config {
    ffi_catch_panic! {
        let config = try_mut_from_ptr!(c);
        unwrap_or_panic!(config.load());
        c
    }
}

/// Load the portage config from a given path, use NULL for the default system paths.
///
/// Returns NULL on error.
///
/// # Safety
/// The path argument should be a valid path on the system.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_load_portage_conf(
    c: *mut Config,
    path: *const c_char,
) -> *mut Config {
    ffi_catch_panic! {
        let path = try_opt_str_from_ptr!(path);
        let config = try_mut_from_ptr!(c);
        unwrap_or_panic!(config.load_portage_conf(path));
        c
    }
}

/// Return the repos for a config.
///
/// # Safety
/// The config argument must be a non-null Config pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_repos(
    c: *mut Config,
    len: *mut usize,
) -> *mut *const Repo {
    // TODO: switch from usize to std::os::raw::c_size_t when it's stable.
    let config = try_ref_from_ptr!(c);
    iter_to_array!(config.repos.into_iter(), len, |(_, r)| { r as *const _ })
}

/// Return the RepoSet for a given set type.
///
/// # Safety
/// The config argument must be a non-null Config pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_repos_set(
    c: *mut Config,
    set_type: RepoSetType,
) -> *mut RepoSet {
    let config = try_ref_from_ptr!(c);
    Box::into_raw(Box::new(config.repos.set(set_type)))
}

/// Free a config.
///
/// # Safety
/// The argument must be a Config pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_free(c: *mut Config) {
    if !c.is_null() {
        unsafe { drop(Box::from_raw(c)) };
    }
}
