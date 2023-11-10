use scallop::variables::var_to_vec;
use scallop::ExecStatus;

use crate::shell::builtins::{eapply, eapply_user, einstalldocs};
use crate::shell::BuildData;

use super::emake_install;

pub(crate) fn src_prepare(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if let Ok(patches) = var_to_vec("PATCHES") {
        if !patches.is_empty() {
            // Note that not allowing options in PATCHES is technically from EAPI 8, but it's
            // backported here for EAPI 6 onwards.
            let args: Vec<_> = ["--"]
                .into_iter()
                .chain(patches.iter().map(|s| s.as_str()))
                .collect();
            eapply(&args)?;
        }
    }

    eapply_user(&[])
}

pub(crate) fn src_install(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    emake_install(build)?;
    einstalldocs(&[])
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::eapi;
    use crate::macros::assert_err_re;
    use crate::pkg::BuildPackage;
    use crate::shell::get_build_mut;
    use crate::shell::test::FileTree;

    use super::*;

    #[test]
    fn test_src_prepare() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        let file_content = indoc::indoc! {"
            0
        "};
        let patch1 = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -0
            +1
        "};
        let patch2 = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -1
            +2
        "};

        for eapi in eapi::range("6..").unwrap() {
            // no options in PATCHES
            for s in ["( -p1 1.patch )", "\"-p1 1.patch\""] {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="no options in PATCHES"
                    SLOT=0
                    PATCHES={s}
                "#};
                let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                BuildData::from_pkg(&pkg);
                let r = pkg.build();
                assert_err_re!(r, "nonexistent file: -p1$");
            }

            // PATCHES empty
            for s in ["()", "\"\"", ""] {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="PATCHES empty"
                    SLOT=0
                    PATCHES={s}
                "#};
                let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                BuildData::from_pkg(&pkg);
                let _file_tree = FileTree::new();
                pkg.build().unwrap();
                assert!(get_build_mut().user_patches_applied);
            }

            // PATCHES applied
            for s in ["( 1.patch 2.patch )", "\"1.patch 2.patch\""] {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="PATCHES applied"
                    SLOT=0
                    PATCHES={s}
                "#};
                let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                BuildData::from_pkg(&pkg);
                let _file_tree = FileTree::new();
                fs::write("file.txt", file_content).unwrap();
                fs::write("1.patch", patch1).unwrap();
                fs::write("2.patch", patch2).unwrap();
                pkg.build().unwrap();
                assert_eq!(fs::read_to_string("file.txt").unwrap(), "2\n");
            }
        }
    }

    #[test]
    fn test_src_install() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // default src_install handles DOCS and HTML_DOCS
        for eapi in eapi::range("6..").unwrap() {
            for (s1, s2) in [("( a.txt )", "( a.html )"), ("\"a.txt\"", "\"a.html\"")] {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="src_install installing docs"
                    SLOT=0
                    DOCS={s1}
                    HTML_DOCS={s2}
                "#};
                let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                BuildData::from_pkg(&pkg);
                let file_tree = FileTree::new();
                fs::write("a.txt", "data").unwrap();
                fs::write("a.html", "data").unwrap();
                pkg.build().unwrap();
                file_tree.assert(
                    r#"
                    [[files]]
                    path = "/usr/share/doc/pkg-1/a.txt"
                    [[files]]
                    path = "/usr/share/doc/pkg-1/html/a.html"
                "#,
                );
            }
        }
    }
}
