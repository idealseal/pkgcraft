use pkgcraft::dep::parse::restrict_dependency;
use pkgcraft::dep::DependencySet;
use pkgcraft::pkg::ebuild::iuse::Iuse;
use pkgcraft::pkg::ebuild::Pkg;

use crate::report::{
    Report,
    ReportKind::{self, RestrictMissing},
};

pub(super) static REPORTS: &[ReportKind] = &[RestrictMissing];

#[derive(Debug)]
pub(crate) struct Check {
    restricts: DependencySet<String>,
    iuse: Iuse,
}

impl Check {
    pub(super) fn new() -> Self {
        Self {
            restricts: ["test", "!test? ( test )"]
                .iter()
                .map(|s| {
                    restrict_dependency(s).unwrap_or_else(|e| panic!("invalid RESTRICT: {s}: {e}"))
                })
                .collect(),
            iuse: Iuse::try_new("test").unwrap(),
        }
    }
}

impl super::CheckRun<&Pkg<'_>> for Check {
    fn run<F: FnMut(Report)>(&self, pkg: &Pkg<'_>, mut report: F) {
        if pkg.iuse().contains(&self.iuse)
            && pkg
                .restrict()
                .intersection(&self.restricts)
                .next()
                .is_none()
        {
            let message = r#"missing RESTRICT="!test? ( test )" with IUSE=test'"#;
            report(RestrictMissing.version(pkg, message));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::check::CheckKind::RestrictTestMissing;
    use crate::scanner::Scanner;
    use crate::test::*;

    #[test]
    fn check() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(RestrictTestMissing);
        let scanner = Scanner::new().jobs(1).checks([RestrictTestMissing]);
        let expected = glob_reports!("{check_dir}/*/reports.json");

        // check dir restriction
        let restrict = repo.restrict_from_path(&check_dir).unwrap();
        let reports: Vec<_> = scanner.run(repo, [&restrict]).collect();
        assert_eq!(&reports, &expected);

        // repo restriction
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);
    }

    #[test]
    fn patched() {
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let scanner = Scanner::new().jobs(1).checks([RestrictTestMissing]);
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}