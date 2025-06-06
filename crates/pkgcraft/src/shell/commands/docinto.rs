use scallop::ExecStatus;

use crate::shell::environment::Variable::DOCDESTTREE;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "docinto",
    disable_help_flag = true,
    long_about = indoc::indoc! {"
        Takes exactly one argument and sets the install path for dodoc and other
        doc-related commands.
    "}
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    path: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    DOCDESTTREE.set(cmd.path)?;
    Ok(ExecStatus::Success)
}

make_builtin!("docinto", docinto_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::BuildData;
    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, docinto, dodoc};

    cmd_scope_tests!("docinto /install/path");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(docinto, &[0, 2]);
    }

    #[test]
    fn creation() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        fs::File::create("file").unwrap();

        docinto(&["examples"]).unwrap();
        dodoc(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/examples/file"
        "#,
        );

        docinto(&["/"]).unwrap();
        dodoc(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/file"
        "#,
        );

        docinto(&["-"]).unwrap();
        dodoc(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/-/file"
        "#,
        );
    }
}
