use scallop::ExecStatus;

use super::_query_cmd::query_cmd;
use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "Determine if a package dependency is installed.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let mut cpvs = query_cmd(args)?;
    if cpvs.next().is_some() {
        Ok(ExecStatus::Success)
    } else {
        Ok(ExecStatus::Failure(1))
    }
}

const USAGE: &str = "has_version 'cat/pkg[use]'";
make_builtin!("has_version", has_version_builtin, run, LONG_DOC, USAGE, [("..", [Phases])]);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as has_version;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(has_version, &[0]);
    }

    // TODO: add usage tests
}
