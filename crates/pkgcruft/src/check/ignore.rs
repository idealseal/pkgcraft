use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};

use crate::report::{ReportKind::IgnoreUnused, ReportScope};
use crate::scan::ScannerRun;

use super::{CategoryCheck, CpnCheck, CpvCheck};

static CHECK: super::Check = super::Check::Ignore;

pub(super) struct Check;

super::register!(Check);

impl CpvCheck for Check {
    fn run(&self, _cpv: &Cpv, _run: &ScannerRun) {}
    fn finish_target(&self, cpv: &Cpv, run: &ScannerRun) {
        let scope = ReportScope::Version(cpv.clone(), None);

        // forciby populate the cache
        run.ignore.generate(&scope).count();

        // flag unused version scope ignore directives
        if let Some(sets) = run.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.version(cpv).message(sets).report(run);
        }
    }

    fn finish_check(&self, run: &ScannerRun) {
        let scope = ReportScope::Repo(run.repo.to_string());
        if let Some(sets) = run.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.repo(&run.repo).message(sets).report(run);
        }
    }
}

impl CpnCheck for Check {
    fn run(&self, _cpn: &Cpn, _run: &ScannerRun) {}
    fn finish_target(&self, cpn: &Cpn, run: &ScannerRun) {
        let scope = ReportScope::Package(cpn.clone());

        // forciby populate the cache
        run.ignore.generate(&scope).count();

        // flag unused package scope ignore directives
        if let Some(sets) = run.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.package(cpn).message(sets).report(run);
        }
    }
}

impl CategoryCheck for Check {
    fn run(&self, _category: &str, _run: &ScannerRun) {}
    fn finish_target(&self, category: &str, run: &ScannerRun) {
        let scope = ReportScope::Category(category.to_string());

        // forciby populate the cache
        run.ignore.generate(&scope).count();

        if let Some(sets) = run.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.category(category).message(sets).report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::report::ReportSet;
    use crate::scan::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/IgnoreUnused.json");

        // check isn't run by default
        let scanner = Scanner::new();
        let mut reports = scanner.run(repo, repo).unwrap();
        assert!(!reports.any(|r| CHECK.reports().contains(&r.kind)));

        // check run when all supported reports targeted
        let scanner = Scanner::new().reports([ReportSet::All]);
        let reports: Vec<_> = scanner
            .run(repo, repo)
            .unwrap()
            .filter(|x| x.kind == IgnoreUnused)
            .collect();
        assert_ordered_eq!(&reports, &expected);

        // verify reports in version scope
        let reports: Vec<_> = scanner
            .run(repo, "Ignore/IgnoreUnused-0")
            .unwrap()
            .collect();
        assert_ordered_eq!(&reports, &expected[..1]);

        // verify reports in package scope
        let reports: Vec<_> = scanner.run(repo, "Ignore/IgnoreUnused").unwrap().collect();
        assert_ordered_eq!(&reports, &expected[..2]);

        // verify reports in category scope
        let reports: Vec<_> = scanner.run(repo, "Ignore/*").unwrap().collect();
        assert_ordered_eq!(&reports, &expected[..3]);
    }
}
