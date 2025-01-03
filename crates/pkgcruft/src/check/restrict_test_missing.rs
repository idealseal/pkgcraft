use pkgcraft::dep::{Dependency, DependencySet};
use pkgcraft::pkg::ebuild::iuse::Iuse;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::restrict::Scope;

use crate::iter::ReportFilter;
use crate::report::ReportKind::RestrictMissing;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::RestrictTestMissing,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[RestrictMissing],
    context: &[],
};

pub(super) fn create() -> impl EbuildPkgCheck {
    Check {
        restricts: ["test", "!test? ( test )"]
            .iter()
            .map(|s| {
                Dependency::restrict(s)
                    .unwrap_or_else(|e| unreachable!("invalid RESTRICT: {s}: {e}"))
            })
            .collect(),
        iuse: Iuse::try_new("test").unwrap(),
    }
}

struct Check {
    restricts: DependencySet<String>,
    iuse: Iuse,
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        if pkg.iuse().contains(&self.iuse)
            && pkg
                .restrict()
                .intersection(&self.restricts)
                .next()
                .is_none()
        {
            RestrictMissing
                .version(pkg)
                .message(r#"missing RESTRICT="!test? ( test )" with IUSE=test"#)
                .report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scan::Scanner;
    use crate::test::*;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
