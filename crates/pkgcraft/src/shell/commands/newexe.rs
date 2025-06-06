use scallop::ExecStatus;

use super::_new::new;
use super::doexe;
use super::make_builtin;

// TODO: convert to clap parser
//const LONG_DOC: &str = "Install renamed environment files into /etc/env.d/.";

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, doexe)
}

make_builtin!("newexe", newexe_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::io::stdin;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, cmd_scope_tests, exeinto, exeopts, newexe};

    cmd_scope_tests!("newexe path/to/executable new_filename");

    #[test]
    fn invalid_args() {
        assert_invalid_args(newexe, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("bin").unwrap();
        newexe(&["bin", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/pkgcraft"
            mode = 0o100755
        "#,
        );

        // explicit root dir
        exeinto(&["/"]).unwrap();
        newexe(&["bin", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/pkgcraft"
            mode = 0o100755
        "#,
        );

        // custom install dir
        exeinto(&["/bin"]).unwrap();
        newexe(&["bin", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/bin/pkgcraft"
            mode = 0o100755
        "#,
        );

        // custom mode and install dir using data from stdin
        stdin().inject("pkgcraft").unwrap();
        exeinto(&["/opt/bin"]).unwrap();
        exeopts(&["-m0777"]).unwrap();
        newexe(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/opt/bin/pkgcraft"
            mode = 0o100777
            data = "pkgcraft"
        "#,
        );
    }
}
