use std::io::Write;

use scallop::{Error, ExecStatus};

use crate::eapi::Feature::UsevTwoArgs;
use crate::io::stdout;
use crate::shell::get_build_mut;

use super::{make_builtin, use_};

const LONG_DOC: &str = "\
The same as use, but also prints the flag name if the condition is met.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let eapi = get_build_mut().eapi();
    let (flag, value) = match args[..] {
        [flag] => (flag, flag.strip_prefix('!').unwrap_or(flag)),
        [flag, value] if eapi.has(UsevTwoArgs) => (flag, value),
        [_, _] => return Err(Error::Base("requires 1 arg, got 2".into())),
        _ => return Err(Error::Base(format!("requires 1 or 2 args, got {}", args.len()))),
    };

    let ret = use_(&[flag])?;
    if bool::from(&ret) {
        write!(stdout(), "{value}")?;
    }

    Ok(ret)
}

const USAGE: &str = "usev flag";
make_builtin!("usev", usev_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::TEST_DATA;

    use super::super::{assert_invalid_args, cmd_scope_tests, usev};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(usev, &[0, 3]);

        for eapi in EAPIS_OFFICIAL.iter().filter(|e| !e.has(UsevTwoArgs)) {
            BuildData::empty(eapi);
            assert_invalid_args(usev, &[2]);
        }
    }

    #[test]
    fn empty_iuse_effective() {
        let repo = TEST_DATA.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert_err_re!(usev(&["use"]), "^.* not in IUSE$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let pkg = temp.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        for (args, status, expected) in
            [(&["use"], ExecStatus::Failure(1), ""), (&["!use"], ExecStatus::Success, "use")]
        {
            assert_eq!(usev(args).unwrap(), status);
            assert_eq!(stdout().get(), expected);
        }

        // check EAPIs that support two arg variant
        for eapi in EAPIS_OFFICIAL.iter().filter(|e| e.has(UsevTwoArgs)) {
            let pkg = temp
                .create_pkg("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            BuildData::from_pkg(&pkg);

            for (args, status, expected) in [
                (&["use", "out"], ExecStatus::Failure(1), ""),
                (&["!use", "out"], ExecStatus::Success, "out"),
            ] {
                assert_eq!(usev(args).unwrap(), status);
                assert_eq!(stdout().get(), expected);
            }
        }

        // enabled
        let pkg = temp.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);
        get_build_mut().use_.insert("use".to_string());

        for (args, status, expected) in
            [(&["use"], ExecStatus::Success, "use"), (&["!use"], ExecStatus::Failure(1), "")]
        {
            assert_eq!(usev(args).unwrap(), status);
            assert_eq!(stdout().get(), expected);
        }

        // check EAPIs that support two arg variant
        for eapi in EAPIS_OFFICIAL.iter().filter(|e| e.has(UsevTwoArgs)) {
            let pkg = temp
                .create_pkg("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            BuildData::from_pkg(&pkg);
            get_build_mut().use_.insert("use".to_string());

            for (args, status, expected) in [
                (&["use", "out"], ExecStatus::Success, "out"),
                (&["!use", "out"], ExecStatus::Failure(1), ""),
            ] {
                assert_eq!(usev(args).unwrap(), status);
                assert_eq!(stdout().get(), expected);
            }
        }
    }
}
