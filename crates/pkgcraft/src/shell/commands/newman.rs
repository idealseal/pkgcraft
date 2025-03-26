use scallop::ExecStatus;

use super::_new::new;
use super::doman;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed man pages into /usr/share/man.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, doman)
}

const USAGE: &str = "newman path/to/man/page new_filename";
make_builtin!("newman", newman_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::io::stdin;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, cmd_scope_tests, newman};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(newman, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("manpage").unwrap();
        newman(&["manpage", "pkgcraft.1"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/man/man1/pkgcraft.1"
            mode = 0o100644
        "#,
        );

        // re-run using data from stdin
        stdin().inject("pkgcraft").unwrap();
        newman(&["-", "pkgcraft.1"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/man/man1/pkgcraft.1"
            data = "pkgcraft"
            mode = 0o100644
        "#,
        );
    }
}
