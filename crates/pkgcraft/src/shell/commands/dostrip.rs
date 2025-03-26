use camino::Utf8PathBuf;
use scallop::ExecStatus;

use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "dostrip",
    long_about = "Include or exclude paths for symbol stripping."
)]
struct Command {
    #[arg(short = 'x')]
    exclude: bool,
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();

    if cmd.exclude {
        build.strip_exclude.extend(cmd.paths);
    } else {
        build.strip_include.extend(cmd.paths);
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "dostrip /path/to/strip";
make_builtin!("dostrip", dostrip_builtin, true);

#[cfg(test)]
mod tests {
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, dostrip};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(dostrip, &[0]);

        // missing args
        assert!(dostrip(&["-x"]).is_err())
    }

    // TODO: run builds with tests and verify file modifications

    #[test]
    fn include() {
        dostrip(&["/test/path"]).unwrap();
        assert!(get_build_mut()
            .strip_include
            .iter()
            .any(|x| x == "/test/path"));
    }

    #[test]
    fn exclude() {
        dostrip(&["-x", "/test/path"]).unwrap();
        assert!(get_build_mut()
            .strip_exclude
            .iter()
            .any(|x| x == "/test/path"));
    }
}
