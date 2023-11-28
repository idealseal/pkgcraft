use scallop::{Error, ExecStatus};

use super::debug_print;
use super::make_builtin;

const LONG_DOC: &str = "\
Calls debug-print with `$1: entering function` as the first argument and the remaining arguments as
additional arguments.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let s = format!("{}: entering function", args[0]);
    let args = &[&[s.as_str()], &args[1..]].concat();
    debug_print(args)
}

const USAGE: &str = "debug-print-function arg1 arg2";
make_builtin!("debug-print-function", debug_print_function_builtin);

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use crate::config::Config;
    use crate::macros::assert_logs_re;
    use crate::pkg::Source;

    use super::super::{assert_invalid_args, cmd_scope_tests, debug_print_function};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(debug_print_function, &[0]);
    }

    #[traced_test]
    #[test]
    fn eclass() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        let eclass = indoc::indoc! {r#"
            # stub eclass
            e1_func() {
                debug-print-function ${FUNCNAME} "$@"
            }
            e1_func msg 1 2 3
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing debug-print-function"
            SLOT=0
        "#};
        let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", data).unwrap();
        raw_pkg.source().unwrap();
        assert_logs_re!("e1_func: entering function msg 1 2 3$");
    }

    #[traced_test]
    #[test]
    fn global() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        let eclass = indoc::indoc! {r#"
            # stub eclass
            e1_func() {
                debug-print-function ${FUNCNAME} "$@"
            }
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing debug-print-function"
            SLOT=0
            e1_func msg 1 2 3
        "#};
        let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", data).unwrap();
        raw_pkg.source().unwrap();
        assert_logs_re!("e1_func: entering function msg 1 2 3$");
    }
}
