use scallop::ExecStatus;

use super::_new::new;
use super::dolib_so;
use super::make_builtin;

// TODO: convert to clap parser
//const LONG_DOC: &str = "Install renamed shared libraries.";

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dolib_so)
}

make_builtin!("newlib.so", newlib_so_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::io::stdin;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, cmd_scope_tests, into, newlib_so};

    cmd_scope_tests!("newlib.so path/to/lib.so new_filename");

    #[test]
    fn invalid_args() {
        assert_invalid_args(newlib_so, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("lib").unwrap();
        newlib_so(&["lib", "pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft.so"
            mode = 0o100755
        "#,
        );

        // custom install dir using data from stdin
        stdin().inject("pkgcraft").unwrap();
        into(&["/"]).unwrap();
        newlib_so(&["-", "pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/lib/pkgcraft.so"
            data = "pkgcraft"
            mode = 0o100755
        "#,
        );
    }
}
