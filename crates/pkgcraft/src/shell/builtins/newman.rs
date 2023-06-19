use scallop::builtins::ExecStatus;

use crate::shell::phase::PhaseKind::SrcInstall;

use super::_new::new;
use super::doman::run as doman;
use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "Install renamed man pages into /usr/share/man.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, doman)
}

const USAGE: &str = "newman path/to/man/page new_filename";
make_builtin!("newman", newman_builtin, run, LONG_DOC, USAGE, &[("..", &[Phase(SrcInstall)])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::shell::test::FileTree;
    use crate::shell::write_stdin;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as newman;
    use super::*;

    builtin_scope_tests!(USAGE);

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
        "#,
        );

        // re-run using data from stdin
        write_stdin!("pkgcraft");
        newman(&["-", "pkgcraft.1"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/man/man1/pkgcraft.1"
            data = "pkgcraft"
        "#,
        );
    }
}
