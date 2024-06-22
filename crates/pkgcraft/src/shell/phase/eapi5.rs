use is_executable::IsExecutable;
use scallop::ExecStatus;

use crate::shell::commands::{econf, einstalldocs::install_docs_from, emake, unpack};
use crate::shell::utils::{configure, makefile_exists};
use crate::shell::{write_stderr, BuildData};

use super::emake_install;

pub(crate) fn pkg_nofetch(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    // TODO: only output URLs for missing distfiles
    if !build.distfiles.is_empty() {
        let pkg = build.pkg()?;
        write_stderr!("The following files must be manually downloaded for {pkg}:\n")?;
        for url in &build.distfiles {
            write_stderr!("{url}\n")?;
        }
    }

    Ok(ExecStatus::Success)
}

pub(crate) fn src_unpack(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    let args: Vec<_> = build.distfiles.iter().map(|s| s.as_str()).collect();
    if !args.is_empty() {
        unpack(&args)?;
    }

    Ok(ExecStatus::Success)
}

pub(crate) fn src_configure(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if configure().is_executable() {
        econf(&[])?;
    }

    Ok(ExecStatus::Success)
}

pub(crate) fn src_compile(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if makefile_exists() {
        emake(&[])?;
    }

    Ok(ExecStatus::Success)
}

pub(crate) fn src_test(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    for target in ["check", "test"] {
        if emake(&[target, "-n"]).is_ok() {
            return emake(&[target]);
        }
    }

    Ok(ExecStatus::Success)
}

pub(crate) fn src_install(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    emake_install(build)?;
    install_docs_from("DOCS")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::eapi;
    use crate::pkg::Build;
    use crate::shell::test::FileTree;

    use super::*;

    #[test]
    fn test_src_install() {
        let mut config = Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // default src_install only handles DOCS and not HTML_DOCS
        for eapi in eapi::range("..6").unwrap() {
            for (s1, s2) in [("( a.txt )", "( a.html )"), ("\"a.txt\"", "\"a.html\"")] {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="src_install installing docs"
                    SLOT=0
                    DOCS={s1}
                    HTML_DOCS={s2}
                "#};
                let pkg = repo.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                BuildData::from_pkg(&pkg);
                let file_tree = FileTree::new();
                fs::write("a.txt", "data").unwrap();
                fs::write("a.html", "data").unwrap();
                pkg.build().unwrap();
                file_tree.assert(
                    r#"
                    [[files]]
                    path = "/usr/share/doc/pkg-1/a.txt"
                "#,
                );
            }
        }
    }
}
