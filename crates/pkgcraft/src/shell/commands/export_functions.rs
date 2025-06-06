use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "EXPORT_FUNCTIONS",
    disable_help_flag = true,
    long_about = indoc::indoc! {"
        Export stub functions that call the eclass's functions, thereby making them default.
        For example, if ECLASS=base and `EXPORT_FUNCTIONS src_unpack` is called the following
        function is defined:

        src_unpack() { base_src_unpack; }
    "}
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true, value_name = "PHASE")]
    phases: Vec<PhaseKind>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let eclass = build.eclass();
    let eapi = build.eapi();

    for phase in cmd.phases {
        if eapi.phases().contains(&phase) {
            build.eclass_phases.insert(phase, eclass.clone());
        } else {
            return Err(Error::Base(format!("EAPI {eapi}: undefined phase: {phase}")));
        }
    }

    Ok(ExecStatus::Success)
}

make_builtin!("EXPORT_FUNCTIONS", export_functions_builtin);

#[cfg(test)]
mod tests {
    use scallop::variables::optional;

    use crate::config::Config;
    use crate::pkg::{Build, Source};
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, export_functions};

    cmd_scope_tests!("EXPORT_FUNCTIONS src_configure src_compile");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(export_functions, &[0]);
    }

    #[test]
    fn single() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            EXPORT_FUNCTIONS src_compile

            e1_src_compile() {
                VAR=1
            }
        "#};
        temp.create_eclass("e1", eclass).unwrap();

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing EXPORT_FUNCTIONS support"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();

        // verify the function runs
        assert!(optional("VAR").is_none());
        BuildData::from_pkg(&pkg);
        pkg.build().unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
    }

    #[test]
    fn nested() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e2

            e1_src_compile() {
                VAR=1
            }

            EXPORT_FUNCTIONS src_compile
        "#};
        temp.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
        "#};
        temp.create_eclass("e2", eclass).unwrap();

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing EXPORT_FUNCTIONS support"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        // verify the function runs
        assert!(optional("VAR").is_none());
        pkg.build().unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
    }

    #[test]
    fn overridden() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            EXPORT_FUNCTIONS src_compile

            e1_src_compile() {
                die "running e1_src_compile"
            }
        "#};
        temp.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            EXPORT_FUNCTIONS src_compile

            e2_src_compile() {
                VAR=1
            }
        "#};
        temp.create_eclass("e2", eclass).unwrap();

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1 e2
            DESCRIPTION="testing EXPORT_FUNCTIONS support"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        // verify the function runs
        assert!(optional("VAR").is_none());
        pkg.build().unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
    }

    #[test]
    fn invalid_phase() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            EXPORT_FUNCTIONS src_compile invalid_phase

            e1_src_compile() { :; }
            e1_invalid_phase() { :; }
        "#};
        temp.create_eclass("e1", eclass).unwrap();

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing EXPORT_FUNCTIONS support"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        assert!(raw_pkg.source().is_err());
    }

    #[test]
    fn undefined_phase() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            EXPORT_FUNCTIONS src_compile src_configure

            e1_src_compile() { :; }
        "#};
        temp.create_eclass("e1", eclass).unwrap();

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing EXPORT_FUNCTIONS support"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        let r = raw_pkg.source();
        assert_err_re!(r, "e1.eclass: undefined phase function: e1_src_configure$");
    }
}
