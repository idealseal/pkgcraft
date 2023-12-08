use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;
use std::{cmp, fmt};

use indexmap::IndexSet;
use once_cell::sync::Lazy;
use scallop::builtins::Builtin;

use super::get_build_mut;
use super::phase::PhaseKind;
use super::scope::{Scope, Scopes};

mod _default_phase_func;
mod _new;
mod _phases;
mod _query_cmd;
mod _use_conf;
mod adddeny;
mod addpredict;
mod addread;
mod addwrite;
mod assert;
mod best_version;
mod command_not_found_handle;
mod debug_print;
mod debug_print_function;
mod debug_print_section;
mod default;
mod default_pkg_nofetch;
mod default_src_compile;
mod default_src_configure;
mod default_src_install;
mod default_src_prepare;
mod default_src_test;
mod default_src_unpack;
mod die;
mod diropts;
mod dobin;
mod docinto;
mod docompress;
mod doconfd;
mod dodir;
mod dodoc;
mod doenvd;
mod doexe;
mod doheader;
mod dohtml;
mod doinfo;
mod doinitd;
mod doins;
mod dolib;
mod dolib_a;
mod dolib_so;
mod doman;
mod domo;
mod dosbin;
mod dostrip;
mod dosym;
pub(super) mod eapply;
pub(super) mod eapply_user;
mod ebegin;
pub(super) mod econf;
mod eend;
mod eerror;
mod einfo;
mod einfon;
mod einstall;
pub(super) mod einstalldocs;
mod elog;
pub(super) mod emake;
mod eqawarn;
mod ewarn;
mod exeinto;
mod exeopts;
mod export_functions;
mod fowners;
mod fperms;
mod get_libdir;
mod has;
mod has_version;
mod hasq;
mod hasv;
mod in_iuse;
mod inherit;
mod insinto;
mod insopts;
mod into;
mod keepdir;
mod libopts;
mod newbin;
mod newconfd;
mod newdoc;
mod newenvd;
mod newexe;
mod newheader;
mod newinitd;
mod newins;
mod newlib_a;
mod newlib_so;
mod newman;
mod newsbin;
mod nonfatal;
mod unpack;
mod use_;
mod use_enable;
mod use_with;
mod useq;
mod usev;
mod usex;
mod ver_cut;
mod ver_rs;
mod ver_test;

// export builtins for internal use
pub(crate) use adddeny::BUILTIN as adddeny;
pub(crate) use addpredict::BUILTIN as addpredict;
pub(crate) use addread::BUILTIN as addread;
pub(crate) use addwrite::BUILTIN as addwrite;
pub(crate) use assert::BUILTIN as assert;
pub(crate) use best_version::BUILTIN as best_version;
pub(crate) use command_not_found_handle::BUILTIN as command_not_found_handle;
pub(crate) use debug_print::BUILTIN as debug_print;
pub(crate) use debug_print_function::BUILTIN as debug_print_function;
pub(crate) use debug_print_section::BUILTIN as debug_print_section;
pub(crate) use default::BUILTIN as default;
pub(crate) use default_pkg_nofetch::BUILTIN as default_pkg_nofetch;
pub(crate) use default_src_compile::BUILTIN as default_src_compile;
pub(crate) use default_src_configure::BUILTIN as default_src_configure;
pub(crate) use default_src_install::BUILTIN as default_src_install;
pub(crate) use default_src_prepare::BUILTIN as default_src_prepare;
pub(crate) use default_src_test::BUILTIN as default_src_test;
pub(crate) use default_src_unpack::BUILTIN as default_src_unpack;
pub(crate) use die::BUILTIN as die;
pub(crate) use diropts::BUILTIN as diropts;
pub(crate) use dobin::BUILTIN as dobin;
pub(crate) use docinto::BUILTIN as docinto;
pub(crate) use docompress::BUILTIN as docompress;
pub(crate) use doconfd::BUILTIN as doconfd;
pub(crate) use dodir::BUILTIN as dodir;
pub(crate) use dodoc::BUILTIN as dodoc;
pub(crate) use doenvd::BUILTIN as doenvd;
pub(crate) use doexe::BUILTIN as doexe;
pub(crate) use doheader::BUILTIN as doheader;
pub(crate) use dohtml::BUILTIN as dohtml;
pub(crate) use doinfo::BUILTIN as doinfo;
pub(crate) use doinitd::BUILTIN as doinitd;
pub(crate) use doins::BUILTIN as doins;
pub(crate) use dolib::BUILTIN as dolib;
pub(crate) use dolib_a::BUILTIN as dolib_a;
pub(crate) use dolib_so::BUILTIN as dolib_so;
pub(crate) use doman::BUILTIN as doman;
pub(crate) use domo::BUILTIN as domo;
pub(crate) use dosbin::BUILTIN as dosbin;
pub(crate) use dostrip::BUILTIN as dostrip;
pub(crate) use dosym::BUILTIN as dosym;
pub(crate) use eapply::BUILTIN as eapply;
pub(crate) use eapply_user::BUILTIN as eapply_user;
pub(crate) use ebegin::BUILTIN as ebegin;
pub(crate) use econf::BUILTIN as econf;
pub(crate) use eend::BUILTIN as eend;
pub(crate) use eerror::BUILTIN as eerror;
pub(crate) use einfo::BUILTIN as einfo;
pub(crate) use einfon::BUILTIN as einfon;
pub(crate) use einstall::BUILTIN as einstall;
pub(crate) use einstalldocs::BUILTIN as einstalldocs;
pub(crate) use elog::BUILTIN as elog;
pub(crate) use emake::BUILTIN as emake;
pub(crate) use eqawarn::BUILTIN as eqawarn;
pub(crate) use ewarn::BUILTIN as ewarn;
pub(crate) use exeinto::BUILTIN as exeinto;
pub(crate) use exeopts::BUILTIN as exeopts;
pub(crate) use export_functions::BUILTIN as export_functions;
pub(crate) use fowners::BUILTIN as fowners;
pub(crate) use fperms::BUILTIN as fperms;
pub(crate) use get_libdir::BUILTIN as get_libdir;
pub(crate) use has::BUILTIN as has;
pub(crate) use has_version::BUILTIN as has_version;
pub(crate) use hasq::BUILTIN as hasq;
pub(crate) use hasv::BUILTIN as hasv;
pub(crate) use in_iuse::BUILTIN as in_iuse;
pub(crate) use inherit::BUILTIN as inherit;
pub(crate) use insinto::BUILTIN as insinto;
pub(crate) use insopts::BUILTIN as insopts;
pub(crate) use into::BUILTIN as into;
pub(crate) use keepdir::BUILTIN as keepdir;
pub(crate) use libopts::BUILTIN as libopts;
pub(crate) use newbin::BUILTIN as newbin;
pub(crate) use newconfd::BUILTIN as newconfd;
pub(crate) use newdoc::BUILTIN as newdoc;
pub(crate) use newenvd::BUILTIN as newenvd;
pub(crate) use newexe::BUILTIN as newexe;
pub(crate) use newheader::BUILTIN as newheader;
pub(crate) use newinitd::BUILTIN as newinitd;
pub(crate) use newins::BUILTIN as newins;
pub(crate) use newlib_a::BUILTIN as newlib_a;
pub(crate) use newlib_so::BUILTIN as newlib_so;
pub(crate) use newman::BUILTIN as newman;
pub(crate) use newsbin::BUILTIN as newsbin;
pub(crate) use nonfatal::BUILTIN as nonfatal;
pub(crate) use unpack::BUILTIN as unpack;
pub(crate) use use_::BUILTIN as use_;
pub(crate) use use_enable::BUILTIN as use_enable;
pub(crate) use use_with::BUILTIN as use_with;
pub(crate) use useq::BUILTIN as useq;
pub(crate) use usev::BUILTIN as usev;
pub(crate) use usex::BUILTIN as usex;
pub(crate) use ver_cut::BUILTIN as ver_cut;
pub(crate) use ver_rs::BUILTIN as ver_rs;
pub(crate) use ver_test::BUILTIN as ver_test;
// phase stubs
pub(crate) use _phases::PKG_CONFIG as pkg_config_stub;
pub(crate) use _phases::PKG_INFO as pkg_info_stub;
pub(crate) use _phases::PKG_NOFETCH as pkg_nofetch_stub;
pub(crate) use _phases::PKG_POSTINST as pkg_postinst_stub;
pub(crate) use _phases::PKG_POSTRM as pkg_postrm_stub;
pub(crate) use _phases::PKG_PREINST as pkg_preinst_stub;
pub(crate) use _phases::PKG_PRERM as pkg_prerm_stub;
pub(crate) use _phases::PKG_PRETEND as pkg_pretend_stub;
pub(crate) use _phases::PKG_SETUP as pkg_setup_stub;
pub(crate) use _phases::SRC_COMPILE as src_compile_stub;
pub(crate) use _phases::SRC_CONFIGURE as src_configure_stub;
pub(crate) use _phases::SRC_INSTALL as src_install_stub;
pub(crate) use _phases::SRC_PREPARE as src_prepare_stub;
pub(crate) use _phases::SRC_TEST as src_test_stub;
pub(crate) use _phases::SRC_UNPACK as src_unpack_stub;

#[derive(Debug, Clone)]
pub struct Command {
    builtin: Builtin,
    scopes: HashSet<Scope>,
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        self.builtin == other.builtin
    }
}

impl Eq for Command {}

impl Hash for Command {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.builtin.hash(state);
    }
}

impl Ord for Command {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.builtin.cmp(&other.builtin)
    }
}

impl PartialOrd for Command {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Borrow<Builtin> for Command {
    fn borrow(&self) -> &Builtin {
        &self.builtin
    }
}

impl Borrow<str> for Command {
    fn borrow(&self) -> &str {
        self.builtin.borrow()
    }
}

impl Borrow<str> for &Command {
    fn borrow(&self) -> &str {
        self.builtin.borrow()
    }
}

impl AsRef<str> for Command {
    fn as_ref(&self) -> &str {
        self.builtin.as_ref()
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.builtin)
    }
}

// TODO: replace with callable trait implementation if it's ever stabilized
// https://github.com/rust-lang/rust/issues/29625
impl Deref for Command {
    type Target = scallop::builtins::BuiltinFn;

    fn deref(&self) -> &Self::Target {
        &self.builtin.func
    }
}

impl Command {
    pub(crate) fn new<I, S>(builtin: Builtin, scopes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Scopes>,
    {
        Self {
            builtin,
            scopes: scopes.into_iter().flat_map(Into::into).collect(),
        }
    }

    /// Determine if the command is allowed in a given `Scope`.
    pub(crate) fn is_allowed(&self, scope: &Scope) -> bool {
        self.scopes.contains(scope)
            || (scope.is_eclass() && self.scopes.contains(&Scope::Eclass(None)))
    }

    /// Determine if the command is a phase stub.
    pub fn is_phase(&self) -> bool {
        PhaseKind::from_str(self.as_ref()).is_ok()
    }
}

/// Ordered set of all known builtins.
pub(crate) static BUILTINS: Lazy<IndexSet<Builtin>> = Lazy::new(|| {
    [
        adddeny,
        addpredict,
        addread,
        addwrite,
        assert,
        best_version,
        command_not_found_handle,
        debug_print,
        debug_print_function,
        debug_print_section,
        default,
        default_pkg_nofetch,
        default_src_compile,
        default_src_configure,
        default_src_install,
        default_src_prepare,
        default_src_test,
        default_src_unpack,
        die,
        diropts,
        dobin,
        docinto,
        docompress,
        doconfd,
        dodir,
        dodoc,
        doenvd,
        doexe,
        doheader,
        dohtml,
        doinfo,
        doinitd,
        doins,
        dolib,
        dolib_a,
        dolib_so,
        doman,
        domo,
        dosbin,
        dostrip,
        dosym,
        eapply,
        eapply_user,
        ebegin,
        econf,
        eend,
        eerror,
        einfo,
        einfon,
        einstall,
        einstalldocs,
        elog,
        emake,
        eqawarn,
        ewarn,
        exeinto,
        exeopts,
        export_functions,
        fowners,
        fperms,
        get_libdir,
        has,
        has_version,
        hasq,
        hasv,
        in_iuse,
        inherit,
        insinto,
        insopts,
        into,
        keepdir,
        libopts,
        newbin,
        newconfd,
        newdoc,
        newenvd,
        newexe,
        newheader,
        newinitd,
        newins,
        newlib_a,
        newlib_so,
        newman,
        newsbin,
        nonfatal,
        unpack,
        use_,
        use_enable,
        use_with,
        useq,
        usev,
        usex,
        ver_cut,
        ver_rs,
        ver_test,
        // phase stubs
        pkg_config_stub,
        pkg_info_stub,
        pkg_nofetch_stub,
        pkg_postinst_stub,
        pkg_postrm_stub,
        pkg_preinst_stub,
        pkg_prerm_stub,
        pkg_pretend_stub,
        pkg_setup_stub,
        src_compile_stub,
        src_configure_stub,
        src_install_stub,
        src_prepare_stub,
        src_test_stub,
        src_unpack_stub,
    ]
    .into_iter()
    .collect()
});

peg::parser! {
    grammar cmd() for str {
        rule version_separator() -> &'input str
            = s:$([^ 'a'..='z' | 'A'..='Z' | '0'..='9']+) { s }

        rule version_component() -> &'input str
            = s:$(['0'..='9']+) { s }
            / s:$(['a'..='z' | 'A'..='Z']+) { s }

        rule version_element() -> [&'input str; 2]
            = sep:version_separator() comp:version_component()?
            { [sep, comp.unwrap_or_default()] }
            / sep:version_separator()? comp:version_component()
            { [sep.unwrap_or_default(), comp] }

        // Split version strings for ver_rs and ver_cut.
        pub(super) rule version_split() -> Vec<&'input str>
            = vals:version_element()* { vals.into_iter().flatten().collect() }

        // Parse ranges for ver_rs and ver_cut.
        pub(super) rule range(max: usize) -> (usize, usize)
            = start:$(['0'..='9']+) "-" end:$(['0'..='9']+) {?
                if let (Ok(start), Ok(end)) = (start.parse(), end.parse()) {
                    Ok((start, end))
                } else {
                    Err("range value overflow")
                }
            } / start:$(['0'..='9']+) "-" {?
                match start.parse() {
                    Ok(start) if start <= max => Ok((start, max)),
                    Ok(start) => Ok((start, start)),
                    _ => Err("range value overflow"),
                }
            } / start:$(['0'..='9']+) {?
                let start = start.parse().map_err(|_| "range value overflow")?;
                Ok((start, start))
            }
    }
}

// provide public parsing functionality while converting error types
mod parse {
    use crate::error::peg_error;
    use crate::Error;

    use super::cmd;

    pub(super) fn version_split(s: &str) -> crate::Result<Vec<&str>> {
        cmd::version_split(s).map_err(|e| peg_error("invalid version string", s, e))
    }

    pub(super) fn range(s: &str, max: usize) -> crate::Result<(usize, usize)> {
        let (start, end) = cmd::range(s, max).map_err(|e| peg_error("invalid range", s, e))?;
        if end < start {
            return Err(Error::InvalidValue(format!(
                "start of range ({start}) is greater than end ({end})",
            )));
        }
        Ok((start, end))
    }
}

/// Run a command given its name and argument list from bash.
fn run(name: &str, args: *mut scallop::bash::WordList) -> scallop::ExecStatus {
    use scallop::builtins::handle_error;
    use scallop::{traits::IntoWords, Error};

    let build = get_build_mut();
    let eapi = build.eapi();
    let scope = &build.scope;

    // run if enabled for the current build state
    let result = match eapi.commands().get(name) {
        Some(cmd) if cmd.is_allowed(scope) => {
            let args = args.to_words();
            let args: Result<Vec<_>, _> = args.into_iter().collect();
            match args {
                Ok(args) => cmd(&args),
                Err(e) => Err(Error::Base(format!("non-unicode args: {e}"))),
            }
        }
        Some(_) => Err(Error::Base(format!("disabled in {scope} scope"))),
        None => Err(Error::Base(format!("disabled in EAPI {eapi}"))),
    };

    // handle errors, bailing out when running normally
    result.unwrap_or_else(|e| match e {
        Error::Base(s) if !build.nonfatal => handle_error(name, Error::Bail(s)),
        _ => handle_error(name, e),
    })
}

/// Create a static [`Builtin`] object for registry in bash.
#[macro_export]
macro_rules! make_builtin {
    ($name:expr, $func_name:ident) => {
        make_builtin!($name, $func_name, BUILTIN);
    };
    ($name:expr, $func_name:ident, $builtin:ident) => {
        #[no_mangle]
        extern "C" fn $func_name(args: *mut scallop::bash::WordList) -> std::ffi::c_int {
            i32::from($crate::shell::commands::run($name, args))
        }

        pub(crate) static $builtin: scallop::builtins::Builtin = scallop::builtins::Builtin {
            name: $name,
            func: run,
            flags: scallop::builtins::Attr::ENABLED.bits(),
            cfunc: $func_name,
            help: LONG_DOC,
            usage: USAGE,
        };
    };
}
use make_builtin;

#[cfg(test)]
fn assert_invalid_args(builtin: Builtin, nums: &[u32]) {
    for n in nums {
        let args: Vec<_> = (0..*n).map(|n| n.to_string()).collect();
        let args: Vec<_> = args.iter().map(|s| s.as_str()).collect();
        let re = format!("^.*, got {n}");
        crate::macros::assert_err_re!(builtin(&args), re);
    }
}

#[cfg(test)]
macro_rules! cmd_scope_tests {
    ($cmd:expr) => {
        #[test]
        fn cmd_scope() {
            use std::collections::HashSet;

            use crate::config::Config;
            use crate::eapi::EAPIS_OFFICIAL;
            use crate::macros::assert_err_re;
            use crate::pkg::Source;
            use crate::shell::scope::{Scope::*, Scopes};

            let cmd = $cmd;
            let name = cmd.split(' ').next().unwrap();
            let mut config = Config::default();
            let t = config.temp_repo("test", 0, None).unwrap();
            let all_scopes: HashSet<_> = Scopes::All.into_iter().collect();

            for eapi in &*EAPIS_OFFICIAL {
                if let Some(cmd) = eapi.commands().get(name) {
                    for scope in all_scopes.difference(&cmd.scopes) {
                        let info = format!("EAPI={eapi}, scope: {scope}");
                        match scope {
                            Eclass(_) => {
                                // create eclass
                                let eclass = indoc::formatdoc! {r#"
                                    # stub eclass
                                    VAR=1
                                    {cmd}
                                    VAR=2
                                "#};
                                t.create_eclass("e1", &eclass).unwrap();
                                let data = indoc::formatdoc! {r#"
                                    EAPI={eapi}
                                    inherit e1
                                    DESCRIPTION="testing eclass scope failures"
                                    SLOT=0
                                "#};
                                let raw_pkg =
                                    t.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
                                let r = raw_pkg.source();
                                // verify sourcing stops at unknown command
                                assert_eq!(scallop::variables::optional("VAR").unwrap(), "1");
                                // verify error output
                                let err = format!("{name}: error: disabled in eclass scope");
                                assert_err_re!(r, err, &info);
                            }
                            Global => {
                                let data = indoc::formatdoc! {r#"
                                    EAPI={eapi}
                                    DESCRIPTION="testing global scope failures"
                                    SLOT=0
                                    LICENSE="MIT"
                                    VAR=1
                                    {cmd}
                                    VAR=2
                                "#};
                                let raw_pkg =
                                    t.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
                                let r = raw_pkg.source();
                                // verify sourcing stops at unknown command
                                assert_eq!(scallop::variables::optional("VAR").unwrap(), "1");
                                // verify error output
                                let err = format!("{name}: error: disabled in global scope");
                                assert_err_re!(r, err, &info);
                            }
                            Phase(phase) => {
                                let data = indoc::formatdoc! {r#"
                                    EAPI={eapi}
                                    DESCRIPTION="testing phase scope failures"
                                    SLOT=0
                                    VAR=1
                                    {phase}() {{
                                        {cmd}
                                        VAR=2
                                    }}
                                "#};
                                let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                                pkg.source().unwrap();
                                let phase = eapi.phases().get(phase).unwrap();
                                let r = phase.run();
                                // verify function stops at unknown command
                                assert_eq!(
                                    scallop::variables::optional("VAR").as_deref(),
                                    Some("1")
                                );
                                // verify error output
                                let err = format!("{name}: error: disabled in {phase} scope");
                                assert_err_re!(r, err, &info);
                            }
                        }
                    }
                } else {
                    let data = indoc::formatdoc! {r#"
                        EAPI={eapi}
                        DESCRIPTION="testing command disabled in EAPI failures"
                        SLOT=0
                        LICENSE="MIT"
                        VAR=1
                        {cmd}
                        VAR=2
                    "#};
                    let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
                    let r = raw_pkg.source();
                    // verify sourcing stops at unknown command
                    assert_eq!(scallop::variables::optional("VAR").unwrap(), "1");
                    // verify error output
                    let err = format!("{name}: error: disabled in EAPI {eapi}");
                    assert_err_re!(r, err);
                }
            }
        }
    };
}
#[cfg(test)]
use cmd_scope_tests;
