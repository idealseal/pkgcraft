use camino::Utf8Path;
use itertools::Itertools;
use pkgcraft::files::is_ebuild;
use pkgcraft::macros::build_path;
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};

use crate::report::ReportKind::{RepoCategoriesUnused, RepoCategoryEmpty, RepoPackageEmpty};
use crate::scan::ScannerRun;

use super::RepoCheck;

pub(super) fn create() -> impl RepoCheck {
    Check
}

static CHECK: super::Check = super::Check::RepoLayout;

struct Check;

super::register!(Check);

/// Determine if an ebuild file exists in a directory path.
fn find_ebuild(path: &Utf8Path) -> bool {
    path.read_dir_utf8()
        .map(|entries| entries.filter_map(Result::ok).any(|e| is_ebuild(&e)))
        .unwrap_or(false)
}

impl RepoCheck for Check {
    fn run(&self, repo: &EbuildRepo, run: &ScannerRun) {
        // verify inherited categories
        for category in repo.categories() {
            let mut pkgs = vec![];
            for pkg in repo.packages(&category) {
                let path = build_path!(repo, &category, &pkg);
                if !find_ebuild(&path) {
                    RepoPackageEmpty.package((&category, &pkg)).report(run);
                } else {
                    pkgs.push(pkg);
                }
            }
            if pkgs.is_empty() {
                RepoCategoryEmpty.category(&category).report(run);
            }
        }

        // verify metadata categories
        let unused = repo
            .metadata()
            .categories()
            .iter()
            .filter(|x| !repo.path().join(x).is_dir())
            .join(", ");
        if !unused.is_empty() {
            RepoCategoriesUnused.repo(repo).message(unused).report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scan::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path();
        let expected = glob_reports!("{dir}/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
