use scallop::{Error, ExecStatus};

use crate::eapi::Feature::NonfatalDie;
use crate::shell::{get_build_mut, write_stderr};

use super::make_builtin;

const LONG_DOC: &str = "\
Displays a failure message provided in an optional argument and then aborts the build process.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let build = get_build_mut();
    let supported = build.eapi().has(NonfatalDie);

    let (nonfatal, msg) = match args[..] {
        ["-n", msg] if supported => (build.nonfatal, msg),
        ["-n"] if supported => (build.nonfatal, ""),
        [msg] => (false, msg),
        [] => (false, "(no error message)"),
        _ => return Err(Error::Base(format!("takes up to 1 arg, got {}", args.len()))),
    };

    if nonfatal {
        if !msg.is_empty() {
            write_stderr!("{msg}\n")?;
        }
        Ok(ExecStatus::Failure(1))
    } else {
        // TODO: add bash backtrace to output
        Err(Error::Bail(msg.to_string()))
    }
}

const USAGE: &str = "die \"error message\"";
make_builtin!("die", die_builtin);

#[cfg(test)]
mod tests {
    use scallop::source;
    use scallop::variables::{self, *};

    use crate::config::Config;
    use crate::eapi::{EAPI5, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkg::Build;
    use crate::shell::phase::PhaseKind;
    use crate::shell::{BuildData, BuildState, Scope};

    use super::super::{assert_invalid_args, cmd_scope_tests, die};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(die, &[3]);

        for eapi in EAPIS_OFFICIAL.iter().filter(|e| !e.has(NonfatalDie)) {
            BuildData::empty(eapi);
            assert_invalid_args(die, &[2]);
        }
    }

    #[test]
    fn main() {
        bind("VAR", "1", None, None).unwrap();

        let r = source::string("die && VAR=2");
        assert_err_re!(r, r"^line 1: die: error: \(no error message\)$");

        // verify bash state
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // verify message output
        let r = source::string("die \"output message\"");
        assert_err_re!(r, "^line 1: die: error: output message$");
    }

    #[test]
    fn subshell() {
        bind("VAR", "1", None, None).unwrap();

        // forced subshell
        let r = source::string("(die msg); VAR=2");
        assert_err_re!(r, "^line 1: die: error: msg$");
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // command substitution
        let r = source::string("VAR=$(die msg); VAR=2");
        assert_err_re!(r, "^line 1: die: error: msg$");
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // process substitution
        let r = source::string("echo >$(die msg); VAR=2");
        assert_err_re!(r, "^line 1: die: error: msg$");
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // no message
        let r = source::string("VAR=$(die)");
        assert_err_re!(r, r"^line 1: die: error: \(no error message\)$");

        // verify failure during build
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        for eapi in &*EAPIS_OFFICIAL {
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="subshell die"
                SLOT=0
                pkg_setup() {{
                    local var=$(die subshell)
                    die main
                }}
            "#};
            let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
            BuildData::from_pkg(&pkg);
            let result = pkg.build();
            assert_err_re!(result, "line 5: die: error: subshell$");
        }
    }

    #[test]
    fn nonfatal() {
        let build = get_build_mut();
        build.scope = Scope::Phase(PhaseKind::SrcInstall);
        build.state = BuildState::Empty(&EAPI5);
        bind("VAR", "1", None, None).unwrap();

        // `die -n` only works in supported EAPIs
        let r = source::string("die -n");
        assert_err_re!(r, "^line 1: die: error: -n");

        build.state = BuildState::Empty(EAPIS_OFFICIAL.last().unwrap());

        // `die -n` only works as expected when run with nonfatal
        let r = source::string("die -n message");
        assert_err_re!(r, "^line 1: die: error: message");

        // nonfatal requires `die -n` call
        let r = source::string("nonfatal die");
        assert_err_re!(r, r"line 1: die: error: \(no error message\)$");

        // nonfatal die in main process
        bind("VAR", "1", None, None).unwrap();
        source::string("nonfatal die -n message; VAR=2").unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "2");

        // nonfatal die in subshell without message
        bind("VAR", "1", None, None).unwrap();
        source::string("MSG=$(nonfatal die -n); VAR=2").unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "2");
        assert_eq!(variables::optional("MSG").unwrap(), "");

        // nonfatal die in subshell with message
        bind("VAR", "1", None, None).unwrap();
        source::string("MSG=$(nonfatal die -n message 2>&1); VAR=2").unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "2");
        assert_eq!(variables::optional("MSG").unwrap(), "message");
    }
}
