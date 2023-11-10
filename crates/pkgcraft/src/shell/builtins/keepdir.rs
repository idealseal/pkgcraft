use std::fs::File;

use scallop::{Error, ExecStatus};

use crate::pkg::Package;
use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Install directories.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    let install = build.install();

    // use custom file name including pkg info
    let pkg = build.pkg()?;
    let (cat, pkg, slot) = (pkg.cpv().category(), pkg.cpv().package(), pkg.slot());
    let file_name = format!(".keep_{cat}_{pkg}_{slot}");

    // create dirs
    install.dirs(args)?;

    // create stub files
    for path in args {
        let keep = install.prefix(path).join(&file_name);
        File::create(&keep)
            .map_err(|e| Error::Base(format!("failed creating keep file: {keep:?}: {e}")))?;
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "keepdir path/to/kept/dir";
make_builtin!("keepdir", keepdir_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::BuildPackage;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests, keepdir};
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(keepdir, &[0]);
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        for dirs in [
            vec!["dir"],
            vec!["path/to/dir"],
            vec!["/etc"],
            vec!["/usr/bin"],
            vec!["dir", "/usr/bin"],
        ] {
            let args = dirs.join(" ");
            let data = indoc::formatdoc! {r#"
                EAPI=8
                DESCRIPTION="testing keepdir"
                SLOT=0
                src_install() {{
                    keepdir {args}
                }}
            "#};
            let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
            BuildData::from_pkg(&pkg);
            let file_tree = FileTree::new();
            pkg.build().unwrap();

            let mut files = vec![];
            for dir in dirs {
                let path = dir.trim_start_matches('/');
                files.push(format!(
                    r#"
                    [[files]]
                    path = "/{path}/.keep_cat_pkg_0"
                    mode = 0o100644
                    data = ""
                "#
                ));
            }
            file_tree.assert(files.join("\n"));
        }
    }
}
