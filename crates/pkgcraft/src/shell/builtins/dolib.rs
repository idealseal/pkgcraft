use itertools::Either;
use scallop::{Error, ExecStatus};

use crate::macros::build_from_paths;
use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;
use crate::shell::utils::get_libdir;

use super::make_builtin;

const LONG_DOC: &str = "Install libraries.";

pub(super) fn install_lib(args: &[&str], opts: Option<&[&str]>) -> scallop::Result<ExecStatus> {
    let build = get_build_mut();
    let libdir = get_libdir(Some("lib")).unwrap();
    let dest = build_from_paths!(&build.desttree, &libdir);
    let options = match opts {
        Some(vals) => Either::Left(vals.iter().copied()),
        None => Either::Right(build.libopts.iter().map(|s| s.as_str())),
    };
    let install = build.install().dest(dest)?.file_options(options);
    install.files(args)?;

    Ok(ExecStatus::Success)
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    install_lib(args, None)
}

const USAGE: &str = "dolib path/to/lib";
make_builtin!("dolib", dolib_builtin, run, LONG_DOC, USAGE, [("..7", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;

    use super::super::into::run as into;
    use super::super::libopts::run as libopts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dolib;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dolib, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = dolib(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100644;
        let custom_mode = 0o100755;

        fs::File::create("pkgcraft").unwrap();
        dolib(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft"
            mode = {default_mode}
        "#
        ));

        // custom mode and install dir
        into(&["/"]).unwrap();
        libopts(&["-m0755"]).unwrap();
        dolib(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/lib/pkgcraft"
            mode = {custom_mode}
        "#
        ));
    }
}
