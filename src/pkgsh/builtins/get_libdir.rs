use std::io::{stdout, Write};

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::string_value;
use scallop::{Error, Result};

use super::{PkgBuiltin, GLOBAL};
use crate::macros::write_flush;

static LONG_DOC: &str = "Output the libdir name.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Builtin(format!("takes no args, got {}", args.len())));
    }

    let mut libdir = String::from("lib");
    if let Some(abi) = string_value("ABI") {
        if let Some(val) = string_value(format!("LIBDIR_{}", abi)) {
            libdir = val;
        }
    }

    write_flush!(stdout(), "{}", libdir);
    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "get_libdir",
            func: run,
            help: LONG_DOC,
            usage: "get_libdir",
        },
        &[("6-", &[GLOBAL])],
    )
});

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::super::assert_invalid_args;
    use super::run as get_libdir;

    use gag::BufferRedirect;
    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;
    use scallop::variables::*;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(get_libdir, &[1]);
        }

        #[test]
        fn default() {
            let mut buf = BufferRedirect::stdout().unwrap();
            let mut abi_var = Variable::new("ABI");
            for abi in [None, Some(""), Some("abi")] {
                if let Some(val) = abi {
                    abi_var.bind(val, None, None).unwrap();
                }
                assert_eq!(get_libdir(&[]).unwrap(), ExecStatus::Success);
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(output, "lib");
            }
        }

        #[test]
        fn abi() {
            let mut buf = BufferRedirect::stdout().unwrap();
            let mut abi_var = Variable::new("ABI");
            for (abi, libdir) in [
                    ("abi1", "libabi1"),
                    ("abi2", "libabi2"),
                    ] {
                abi_var.bind(abi, None, None).unwrap();
                bind(format!("LIBDIR_{}", abi), libdir, None, None).unwrap();
                assert_eq!(get_libdir(&[]).unwrap(), ExecStatus::Success);
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(output, libdir);
            }
        }
    }
}
