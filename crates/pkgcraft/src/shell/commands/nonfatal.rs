use scallop::command::{Command, Flags};
use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "\
Takes one or more arguments and executes them as a command, preserving the exit status. If this
results in a command being called that would normally abort the build process due to a failure,
instead a non-zero exit status shall be returned.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    // enable nonfatal status
    let build = get_build_mut();
    build.nonfatal = true;

    // run the specified command
    let cmd = Command::new(args.join(" "), Some(Flags::FORCE_SUBSHELL))?;
    let result = match cmd.execute() {
        r @ (Ok(_) | Err(Error::Bail(_))) => r,
        Err(Error::Status(s)) => Ok(s),
        _ => Ok(ExecStatus::Failure(1)),
    };

    // disable nonfatal status
    build.nonfatal = false;
    result
}

const USAGE: &str = "nonfatal cmd arg1 arg2";
make_builtin!("nonfatal", nonfatal_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, cmd_scope_tests, nonfatal};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(nonfatal, &[0]);
    }

    #[test]
    fn success() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test1", 0, None).unwrap();
        let raw_pkg = temp.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        let status = nonfatal(&["ver_cut", "2-3", "1.2.3"]).unwrap();
        assert_eq!(status, ExecStatus::Success);
    }

    #[test]
    fn exit() {
        let status = nonfatal(&["exit"]).unwrap();
        assert_eq!(status, ExecStatus::Success);

        let status = nonfatal(&["exit 2"]).unwrap();
        assert_eq!(status, ExecStatus::Failure(2));
    }

    #[test]
    fn nonexistent_cmd() {
        let status = nonfatal(&["nonexistent_cmd"]).unwrap();
        assert_eq!(status, ExecStatus::Failure(1));
    }

    #[test]
    fn die() {
        let status = nonfatal(&["die", "-n", "message"]).unwrap();
        assert_eq!(status, ExecStatus::Failure(1));
    }

    #[test]
    fn invalid_builtin_scope() {
        let status = nonfatal(&["best_version", "cat/pkg"]).unwrap();
        assert_eq!(status, ExecStatus::Failure(1));
    }
}
