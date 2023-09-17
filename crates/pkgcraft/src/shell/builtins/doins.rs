use std::path::Path;

use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::files::NO_WALKDIR_FILTER;
use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::make_builtin;

const LONG_DOC: &str = "Install files into INSDESTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (recursive, args) = match args.first().copied() {
        Some("-r") => (true, &args[1..]),
        _ => (false, args),
    };

    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".to_string()));
    }

    let build = get_build_mut();
    let dest = &build.insdesttree;
    let opts = &build.insopts;
    let install = build.install().dest(dest)?.file_options(opts);

    let (dirs, files): (Vec<_>, Vec<_>) = args.iter().map(Path::new).partition(|p| p.is_dir());

    if !dirs.is_empty() {
        if recursive {
            install.recursive(dirs, NO_WALKDIR_FILTER)?;
        } else {
            return Err(Error::Base(format!("non-recursive dir install: {:?}", dirs[0])));
        }
    }

    install.files(files)?;
    Ok(ExecStatus::Success)
}

const USAGE: &str = "doins path/to/file";
make_builtin!("doins", doins_builtin, run, LONG_DOC, USAGE, [("..", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;

    use super::super::insinto::run as insinto;
    use super::super::insopts::run as insopts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as doins;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doins, &[0]);

        // missing args
        let r = doins(&["-r"]);
        assert_err_re!(r, "^requires 1 or more args, got 0");

        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = doins(&["dir"]);
        assert_err_re!(r, "^non-recursive dir install: .*$");

        // nonexistent
        let r = doins(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        let default_mode = 0o100644;
        let custom_mode = 0o100755;

        // simple file
        fs::File::create("file").unwrap();
        doins(&["file"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/file"
            mode = {default_mode}
        "#
        ));

        for dir in ["newdir", "/newdir"] {
            // recursive using `insinto` and `insopts`
            fs::create_dir_all("dir/subdir").unwrap();
            fs::File::create("dir/subdir/file").unwrap();
            insinto(&[dir]).unwrap();
            insopts(&["-m0755"]).unwrap();
            doins(&["-r", "dir"]).unwrap();
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/newdir/dir/subdir/file"
                mode = {custom_mode}
            "#
            ));
        }
    }
}
