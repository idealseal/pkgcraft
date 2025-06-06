use std::collections::HashSet;
use std::fmt;
use std::io::Write;

use camino::Utf8Path;
use scallop::{Error, ExecStatus};
use walkdir::DirEntry;

use crate::io::stderr;
use crate::macros::build_path;
use crate::shell::environment::Variable::DOCDESTTREE;
use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug, Default)]
#[command(
    name = "dohtml",
    disable_help_flag = true,
    long_about = "Install HTML documentation files."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(short = 'r')]
    recursive: bool,

    #[arg(short = 'V')]
    verbose: bool,

    #[arg(short = 'A')]
    extra_file_exts: Vec<String>,

    #[arg(short = 'a', default_value = "css,gif,htm,html,jpeg,jpg,js,png")]
    allowed_file_exts: Vec<String>,

    #[arg(short = 'f')]
    allowed_files: Vec<String>,

    #[arg(short = 'x')]
    excluded_dirs: Vec<String>,

    #[arg(short = 'p')]
    doc_prefix: Option<String>,

    #[arg(required = true, value_name = "TARGET")]
    targets: Vec<String>,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let csv_or_none = |val: &[String]| -> String {
            if val.is_empty() {
                "none".to_string()
            } else {
                val.join(",")
            }
        };

        let s = indoc::formatdoc! {r#"
            dohtml:
              recursive: {}
              verbose: {}
              extra file exts: {}
              allowed file exts: {}
              allowed files: {}
              excluded dirs: {}
              doc prefix: {}
        "#,
        self.recursive,
        self.verbose,
        csv_or_none(&self.extra_file_exts),
        csv_or_none(&self.allowed_file_exts),
        csv_or_none(&self.allowed_files),
        csv_or_none(&self.excluded_dirs),
        self.doc_prefix.as_deref().unwrap_or("none"),
        };

        write!(f, "{s}")
    }
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;

    if cmd.verbose {
        write!(stderr(), "{cmd}")?;
    }

    // TODO: replace csv expansion with clap arg parsing?
    let mut allowed_file_exts: HashSet<_> = cmd
        .allowed_file_exts
        .iter()
        .flat_map(|s| s.split(','))
        .collect();
    allowed_file_exts.extend(cmd.extra_file_exts.iter().flat_map(|s| s.split(',')));
    let excluded_dirs: HashSet<_> = cmd
        .excluded_dirs
        .iter()
        .flat_map(|s| s.split(','))
        .map(Utf8Path::new)
        .collect();
    let allowed_files: HashSet<_> = cmd
        .allowed_files
        .iter()
        .flat_map(|s| s.split(','))
        .collect();

    // determine if a file is allowed
    let allowed_file = |path: &Utf8Path| -> bool {
        match (path.file_name(), path.extension()) {
            (Some(name), Some(ext)) => {
                if allowed_files.is_empty() {
                    allowed_file_exts.contains(ext)
                } else {
                    allowed_files.contains(name)
                }
            }
            _ => false,
        }
    };

    // determine if a walkdir entry is allowed
    let is_allowed = |entry: &DirEntry| -> bool {
        if let Some(path) = Utf8Path::from_path(entry.path()) {
            if path.is_dir() {
                !excluded_dirs.contains(path)
            } else {
                allowed_file(path)
            }
        } else {
            true
        }
    };

    let build = get_build_mut();
    let subdir = match build.env(DOCDESTTREE) {
        "" => "html",
        val => val,
    };
    let doc_prefix = cmd
        .doc_prefix
        .as_ref()
        .map(|s| s.trim_start_matches('/'))
        .unwrap_or_default();
    let dest = build_path!("/usr/share/doc", build.cpv().pf(), subdir, doc_prefix);
    let install = build.install().dest(dest)?;

    let (dirs, mut files): (Vec<_>, Vec<_>) = cmd
        .targets
        .iter()
        .map(Utf8Path::new)
        .partition(|p| p.is_dir());

    if let Some(path) = dirs.first() {
        if cmd.recursive {
            install.recursive(dirs, Some(is_allowed))?;
        } else {
            return Err(Error::Base(format!("trying to install directory as file: {path}")));
        }
    }

    files.retain(|f| allowed_file(f));
    install.files(files)?;

    Ok(ExecStatus::Success)
}

make_builtin!("dohtml", dohtml_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::BuildData;
    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, docinto, dohtml};
    use super::*;

    cmd_scope_tests!("dohtml path/to/html/files");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(dohtml, &[0]);

        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = dohtml(&["dir"]);
        assert_err_re!(r, "^trying to install directory as file: .*$");
    }

    #[test]
    fn verbose_output() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let _file_tree = FileTree::new();
        fs::File::create("index.html").unwrap();

        // defaults
        dohtml(&["-V", "index.html"]).unwrap();
        let s = indoc::indoc! {r#"
            dohtml:
              recursive: false
              verbose: true
              extra file exts: none
              allowed file exts: css,gif,htm,html,jpeg,jpg,js,png
              allowed files: none
              excluded dirs: none
              doc prefix: none
        "#};
        assert_eq!(stderr().get(), s);

        // extra options
        dohtml(&["-V", "-A", "svg,tiff", "-p", "docs", "index.html"]).unwrap();
        let s = indoc::indoc! {r#"
            dohtml:
              recursive: false
              verbose: true
              extra file exts: svg,tiff
              allowed file exts: css,gif,htm,html,jpeg,jpg,js,png
              allowed files: none
              excluded dirs: none
              doc prefix: docs
        "#};
        assert_eq!(stderr().get(), s);
    }

    #[test]
    fn creation() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();

        // simple file
        fs::File::create("index.html").unwrap();
        dohtml(&["index.html"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/index.html"
        "#,
        );

        // recursive
        fs::create_dir_all("doc/subdir").unwrap();
        fs::File::create("doc/subdir/index.html").unwrap();
        dohtml(&["-r", "doc"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/doc/subdir/index.html"
        "#,
        );

        // recursive using `docinto`
        docinto(&["newdir"]).unwrap();
        dohtml(&["-r", "doc"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/newdir/doc/subdir/index.html"
        "#,
        );
    }

    #[test]
    fn options() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();

        fs::create_dir("doc").unwrap();
        fs::File::create("doc/readme.html").unwrap();
        fs::File::create("doc/readme.txt").unwrap();

        // ignored files
        dohtml(&["-r", "doc/."]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.html"
        "#,
        );

        // -A: extra allowed file exts
        dohtml(&["-r", "doc/.", "-A", "txt,md"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.html"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.txt"
        "#,
        );

        // -a: allowed file exts
        dohtml(&["-r", "doc/.", "-a", "txt,md"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.txt"
        "#,
        );

        // -f: allowed files
        dohtml(&["-r", "doc/.", "-f", "readme.txt,readme.md"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.txt"
        "#,
        );

        // -p: doc prefix
        dohtml(&["-r", "doc/.", "-p", "prefix"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/prefix/readme.html"
        "#,
        );

        fs::create_dir("doc/subdir").unwrap();
        fs::File::create("doc/subdir/excluded.html").unwrap();

        // -x: excluded dirs
        dohtml(&["-r", "doc/.", "-x", "doc/subdir,doc/test"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.html"
        "#,
        );
    }
}
