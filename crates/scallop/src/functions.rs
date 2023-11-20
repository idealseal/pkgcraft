use std::ffi::{c_void, CStr, CString};

use crate::error::{ok_or_error, Error};
use crate::macros::*;
use crate::shell::track_top_level;
use crate::{bash, ExecStatus};

#[derive(Debug)]
pub struct Function<'a> {
    func: &'a mut bash::ShellVar,
}

impl Function<'_> {
    pub fn name(&self) -> &str {
        unsafe {
            CStr::from_ptr(self.func.name)
                .to_str()
                .expect("invalid function name")
        }
    }

    /// Execute a given shell function.
    pub fn execute(&mut self, args: &[&str]) -> crate::Result<ExecStatus> {
        let args = [&[self.name()], args].concat();
        let mut args = iter_to_array!(args.iter(), str_to_raw);
        ok_or_error(|| {
            let ret = track_top_level(|| unsafe {
                let words = bash::strvec_to_word_list(args.as_mut_ptr(), 0, 0);
                bash::scallop_execute_shell_function(self.func, words)
            });
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!(
                    "failed running function: {}: exit status {}",
                    self.name(),
                    ret
                )))
            }
        })
    }
}

/// Find a given shell function.
pub fn find<'a, S: AsRef<str>>(name: S) -> Option<Function<'a>> {
    let func_name = CString::new(name.as_ref()).expect("invalid function name");
    let func = unsafe { bash::find_function(func_name.as_ptr()).as_mut() };
    func.map(|f| Function { func: f })
}

/// Run a function in bash function scope.
pub fn bash_func<F>(name: &str, func: F) -> crate::Result<ExecStatus>
where
    F: FnOnce() -> crate::Result<ExecStatus>,
{
    let func_name = CString::new(name).expect("invalid function name");
    unsafe { bash::push_context(func_name.as_ptr() as *mut _, 0, bash::TEMPORARY_ENV) };
    let result = func();
    unsafe { bash::pop_context() };
    result
}

/// Return the names of all visible shell functions.
pub fn all_visible() -> Vec<String> {
    let mut vals = vec![];
    unsafe {
        let shell_vars = bash::all_visible_functions();
        if !shell_vars.is_null() {
            let mut i = 0;
            while let Some(var) = (*shell_vars.offset(i)).as_ref() {
                vals.push(CStr::from_ptr(var.name).to_string_lossy().into());
                i += 1;
            }
            bash::xfree(shell_vars as *mut c_void);
        }
    }
    vals
}

#[cfg(test)]
mod tests {
    use crate::builtins::local;
    use crate::source;
    use crate::variables::{bind, optional};

    use super::*;

    #[test]
    fn find_function() {
        assert!(find("func").is_none());
        source::string("func() { :; }").unwrap();
        let func = find("func").unwrap();
        assert_eq!(func.name(), "func");
    }

    #[test]
    fn execute_success() {
        assert_eq!(optional("VAR"), None);
        source::string("func() { VAR=$1; }").unwrap();
        let mut func = find("func").unwrap();
        func.execute(&[]).unwrap();
        assert_eq!(optional("VAR").unwrap(), "");
        func.execute(&["1"]).unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
    }

    #[test]
    fn execute_failure() {
        source::string("func() { nonexistent_cmd; }").unwrap();
        let mut func = find("func").unwrap();
        assert!(func.execute(&[]).is_err());
    }

    #[test]
    fn bash_func_scope() {
        bind("VAR", "outer", None, None).unwrap();
        bash_func("func_name", || {
            let result = local(["VAR=inner"]);
            assert_eq!(optional("VAR").unwrap(), "inner");
            result
        })
        .unwrap();
        assert_eq!(optional("VAR").unwrap(), "outer");
    }

    #[test]
    fn test_all_visible() {
        assert!(all_visible().is_empty());
        source::string("func() { nonexistent_cmd; }").unwrap();
        assert_eq!(all_visible(), ["func"]);
    }
}
