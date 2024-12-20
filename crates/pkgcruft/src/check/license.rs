use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::{ebuild::EbuildPkg, Package};
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::report::ReportKind::{
    LicenseDeprecated, LicenseMissing, LicenseUnneeded, LicensesUnused,
};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::License,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[LicenseDeprecated, LicenseMissing, LicenseUnneeded, LicensesUnused],
    context: &[],
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        deprecated: repo
            .license_groups()
            .get("DEPRECATED")
            .map(|x| x.iter().collect())
            .unwrap_or_default(),
        unlicensed_categories: ["acct-group", "acct-user", "virtual"]
            .iter()
            .map(|x| x.to_string())
            .collect(),
        unused: repo.metadata().licenses().iter().collect(),
    }
}

struct Check {
    deprecated: IndexSet<&'static String>,
    unlicensed_categories: IndexSet<String>,
    unused: dashmap::DashSet<&'static String>,
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        let licenses: IndexSet<_> = pkg.license().iter_flatten().collect();
        if licenses.is_empty() {
            if !self.unlicensed_categories.contains(pkg.category()) {
                LicenseMissing.version(pkg).report(filter);
            }
        } else if self.unlicensed_categories.contains(pkg.category()) {
            LicenseUnneeded.version(pkg).report(filter);
        } else {
            let deprecated = licenses.intersection(&self.deprecated).sorted().join(", ");
            if !deprecated.is_empty() {
                LicenseDeprecated
                    .version(pkg)
                    .message(deprecated)
                    .report(filter);
            }
        }

        // mangle values for post-run finalization
        if filter.finish() && !self.unused.is_empty() {
            for license in licenses {
                self.unused.remove(license);
            }
        }
    }

    fn finish(&self, repo: &EbuildRepo, filter: &mut ReportFilter) {
        if !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            LicensesUnused.repo(repo).message(unused).report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let check_dir = repo.path().join(CHECK);
        let report_dir = repo.path().join("virtual/LicenseUnneeded");
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{check_dir}/*/reports.json", "{report_dir}/reports.json");
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
