use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::{get_build_mut, BuildVariable};

use super::make_builtin;

const LONG_DOC: &str = "\
Takes exactly one argument and sets the value of DESTTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let path = match args.len() {
        1 => match args[0] {
            "/" => Ok(""),
            s => Ok(s),
        },
        n => Err(Error::Base(format!("requires 1 arg, got {n}"))),
    }?;

    let build = get_build_mut();
    build.desttree = path.to_string();
    build.override_var(BuildVariable::DESTTREE, path)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "into /install/path";
make_builtin!("into", into_builtin, run, LONG_DOC, USAGE, &[("..", &["src_install"])]);

#[cfg(test)]
mod tests {
    use scallop::{shell, variables};

    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::pkgsh::phase::PhaseKind;
    use crate::pkgsh::{BuildData, BuildVariable, Scope};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as into;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(into, &[0]);
    }

    #[test]
    fn set_path() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
        let (_, cpv) = t.create_ebuild("cat/pkg-1", &[]).unwrap();

        for eapi in EAPIS_OFFICIAL.iter() {
            BuildData::update(&cpv, &repo, Some(eapi));
            let phase = PhaseKind::SrcInstall.stub();
            let build = get_build_mut();
            build.scope = Scope::Phase(phase);
            into(&["/test/path"]).unwrap();
            assert_eq!(build.desttree, "/test/path");

            // verify conditional EAPI environment export
            let env_val = variables::optional("DESTTREE");
            match eapi.env().contains_key(&BuildVariable::DESTTREE) {
                true => assert_eq!(env_val.unwrap(), "/test/path"),
                false => assert!(env_val.is_none()),
            }

            // reset shell env
            shell::reset(&[]);
        }
    }
}
