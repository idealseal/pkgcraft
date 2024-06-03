use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::{EbuildPackage, Pkg};
use pkgcraft::repo::{ebuild::Repo, PkgRepository};

use crate::report::ReportKind::DependencySlotMissing;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

pub(super) static CHECK: super::Check = super::Check {
    name: "DependencySlotMissing",
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[DependencySlotMissing],
    context: &[],
    priority: 0,
};

#[derive(Debug)]
pub(crate) struct Check {
    repo: &'static Repo,
}

impl Check {
    pub(crate) fn new(repo: &'static Repo) -> Self {
        Self { repo }
    }
}

impl super::VersionCheckRun for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        for dep in pkg
            .rdepend()
            .intersection(pkg.depend())
            .flat_map(|x| x.iter_flatten())
            .filter(|x| x.blocker().is_none() && x.slot_dep().is_none())
        {
            // TODO: use cached lookup instead of searching for each dep
            let slots = self
                .repo
                .iter_restrict(dep.no_use_deps().as_ref())
                .map(|pkg| pkg.slot().to_string())
                .collect::<IndexSet<_>>();
            if slots.len() > 1 {
                let message = format!("{dep} matches multiple slots: {}", slots.iter().join(", "));
                filter.report(DependencySlotMissing.version(pkg, message));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
