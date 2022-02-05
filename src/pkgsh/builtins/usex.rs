use std::io::{stdout, Write};

use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::{Error, Result};

use super::r#use;
use crate::macros::write_flush;

static LONG_DOC: &str = "\
Tests if a given USE flag is enabled and outputs a string related to its status.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let defaults = ["", "yes", "no", "", ""];
    let (flag, vals) = match args.len() {
        1 => (args[0], defaults),
        2..=5 => {
            // override default values with args
            let stop = args.len();
            let mut vals = defaults;
            vals[1..stop].clone_from_slice(&args[1..stop]);
            (args[0], vals)
        }
        n => return Err(Error::new(format!("requires 1 to 5 args, got {}", n))),
    };

    match r#use::run(&[flag])? {
        ExecStatus::Success => write_flush!(stdout(), "{}{}", vals[1], vals[3]),
        ExecStatus::Failure => write_flush!(stdout(), "{}{}", vals[2], vals[4]),
    }

    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "usex",
    func: run,
    help: LONG_DOC,
    usage: "usex flag",
    error_func: Some(output_error_func),
};

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::super::assert_invalid_args;
    use super::run as usex;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use gag::BufferRedirect;
    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(usex, vec![0, 6]);
        }

        #[test]
        fn empty_iuse_effective() {
            assert_err_re!(usex(&["use"]), "^.* not in IUSE$");
        }

        #[test]
        fn disabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                let mut buf = BufferRedirect::stdout().unwrap();

                // use flag is disabled
                usex(&["use"]).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(&output[..], "no");

                usex(&["use", "arg2", "arg3", "arg4", "arg5"]).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(&output[..], "arg3arg5");

                // inverted check
                usex(&["!use"]).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(&output[..], "yes");

                usex(&["!use", "arg2", "arg3", "arg4", "arg5"]).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(&output[..], "arg2arg4");
            });
        }

        #[test]
        fn enabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                d.borrow_mut().r#use.insert("use".to_string());
                let mut buf = BufferRedirect::stdout().unwrap();

                // use flag is enabled
                usex(&["use"]).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(&output[..], "yes");

                usex(&["use", "arg2", "arg3", "arg4", "arg5"]).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(&output[..], "arg2arg4");

                // inverted check
                usex(&["!use"]).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(&output[..], "no");

                usex(&["!use", "arg2", "arg3", "arg4", "arg5"]).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(&output[..], "arg3arg5");
            });
        }
    }
}
