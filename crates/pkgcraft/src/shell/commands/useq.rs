use scallop::ExecStatus;

use super::make_builtin;
use super::use_;

const LONG_DOC: &str = "Deprecated synonym for use.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    use_(args)
}

const USAGE: &str = "useq flag";
make_builtin!("useq", useq_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::shell::{get_build_mut, BuildData};

    use super::super::{assert_invalid_args, cmd_scope_tests, useq};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(useq, &[0, 2]);
    }

    #[test]
    fn empty_iuse_effective() {
        let mut config = Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();
        let pkg = repo.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);

        assert_err_re!(useq(&["use"]), "^.* not in IUSE$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();
        let pkg = repo.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        assert_eq!(useq(&["use"]).unwrap(), ExecStatus::Failure(1));
        // inverted check
        assert_eq!(useq(&["!use"]).unwrap(), ExecStatus::Success);

        // enabled
        get_build_mut().use_.insert("use".to_string());
        // use flag is enabled
        assert_eq!(useq(&["use"]).unwrap(), ExecStatus::Success);
        // inverted check
        assert_eq!(useq(&["!use"]).unwrap(), ExecStatus::Failure(1));
    }
}
