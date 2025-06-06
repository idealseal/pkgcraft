use std::cmp;
use std::io::Write;

use scallop::ExecStatus;

use crate::io::stdout;
use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin, parse};

#[derive(clap::Parser, Debug)]
#[command(
    name = "ver_cut",
    disable_help_flag = true,
    long_about = "Output substring from package version string and range arguments."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    range: String,
    version: Option<String>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let version = cmd.version.unwrap_or_else(|| get_build_mut().cpv().pv());
    let version_parts = parse::version_split(&version)?;
    let len = version_parts.len();
    let (mut start, mut end) = parse::range(&cmd.range, len / 2)?;

    // remap indices to array positions
    if start != 0 {
        start = cmp::min(start * 2 - 1, len);
    }
    end = cmp::min(end * 2, len);

    let mut stdout = stdout();
    write!(stdout, "{}", &version_parts[start..end].join(""))?;
    stdout.flush()?;

    Ok(ExecStatus::Success)
}

make_builtin!("ver_cut", ver_cut_builtin);

#[cfg(test)]
mod tests {
    use scallop::source;

    use crate::config::Config;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, ver_cut};
    use super::*;

    cmd_scope_tests!("ver_cut 1-2 1.2.3");

    #[test]
    fn invalid_args() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        assert_invalid_cmd(ver_cut, &[0, 3]);
    }

    #[test]
    fn invalid_range() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        for rng in ["-", "-2"] {
            let r = ver_cut(&[rng, "2"]);
            assert!(r.unwrap_err().to_string().contains("invalid range"));
        }

        let r = ver_cut(&["3-2", "1.2.3"]);
        assert_err_re!(r, " is greater than end ");
    }

    #[test]
    fn output() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        // invalid PV
        for (rng, ver, expected) in [
            ("1-2", ".1.2.3", "1.2"),
            ("0-2", ".1.2.3", ".1.2"),
            ("2-3", "1.2.3.", "2.3"),
            ("2-", "1.2.3.", "2.3."),
            ("2-4", "1.2.3.", "2.3."),
        ] {
            temp.create_ebuild("cat/pkg-1.2.3", &[]).unwrap();
            let raw_pkg = repo.get_pkg_raw("cat/pkg-1.2.3").unwrap();
            BuildData::from_raw_pkg(&raw_pkg);

            let r = ver_cut(&[rng, ver]).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);
        }

        // valid PV
        for (rng, ver, expected) in [
            ("1", "1.2.3", "1"),
            ("1-1", "1.2.3", "1"),
            ("1-2", "1.2.3", "1.2"),
            ("2-", "1.2.3", "2.3"),
            ("1-", "1.2.3", "1.2.3"),
            ("3-4", "1.2.3b_alpha4", "3b"),
            ("5", "1.2.3b_alpha4", "alpha"),
            ("0-2", "1.2.3", "1.2"),
            ("2-5", "1.2.3", "2.3"),
            ("4", "1.2.3", ""),
            ("0", "1.2.3", ""),
            ("4-", "1.2.3", ""),
        ] {
            temp.create_ebuild(format!("cat/pkg-{ver}"), &[]).unwrap();
            let raw_pkg = repo.get_pkg_raw(format!("cat/pkg-{ver}")).unwrap();
            BuildData::from_raw_pkg(&raw_pkg);

            let r = ver_cut(&[rng, ver]).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);

            // test pulling version from $PV
            let r = ver_cut(&[rng]).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);
        }
    }

    #[ignore]
    #[test]
    fn subshell() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat/pkg-1.2.3", &[]).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1.2.3").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        source::string("VER=$(ver_cut 2-5 1.2.3)").unwrap();
        assert_eq!(scallop::variables::optional("VER").unwrap(), "2.3");

        // test pulling version from $PV
        source::string("VER=$(ver_cut 1-2)").unwrap();
        assert_eq!(scallop::variables::optional("VER").unwrap(), "1.2");
    }
}
