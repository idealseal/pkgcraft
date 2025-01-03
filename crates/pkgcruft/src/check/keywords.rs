use dashmap::DashSet;
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::pkg::ebuild::{keyword::KeywordStatus::Stable, EbuildPkg};
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::Scope;

use crate::iter::ReportFilter;
use crate::report::ReportKind::{
    ArchesUnused, EapiUnstable, KeywordsLive, KeywordsOverlapping, KeywordsUnsorted,
};
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Keywords,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[
        EapiUnstable,
        KeywordsLive,
        KeywordsOverlapping,
        KeywordsUnsorted,
        ArchesUnused,
    ],
    context: &[],
};

pub(super) fn create(repo: &EbuildRepo, filter: &ReportFilter) -> impl EbuildPkgCheck {
    let unused = if filter.finalize(ArchesUnused) {
        repo.metadata()
            .arches()
            .iter()
            .map(|x| x.to_string())
            .collect()
    } else {
        Default::default()
    };

    Check { repo: repo.clone(), unused }
}

struct Check {
    repo: EbuildRepo,
    unused: DashSet<String>,
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        if !pkg.keywords().is_empty() && pkg.live() {
            KeywordsLive
                .version(pkg)
                .message(pkg.keywords().iter().join(", "))
                .report(filter);
        }

        let mut keywords_map = IndexMap::<_, IndexSet<_>>::new();
        for k in pkg.keywords() {
            // mangle values for post-run finalization
            if filter.finalize(ArchesUnused) {
                self.unused.remove(k.arch().as_ref());
            }

            keywords_map.entry(k.arch()).or_default().insert(k);
        }

        for keywords in keywords_map.values().filter(|k| k.len() > 1) {
            KeywordsOverlapping
                .version(pkg)
                .message(keywords.iter().sorted().join(", "))
                .report(filter);
        }

        let eapi = pkg.eapi().as_str();
        if self.repo.metadata().config.eapis_testing.contains(eapi) {
            let keywords = pkg
                .keywords()
                .iter()
                .filter(|k| k.status() == Stable)
                .sorted()
                .join(" ");
            if !keywords.is_empty() {
                EapiUnstable
                    .version(pkg)
                    .message(format!("unstable EAPI {eapi} with stable keywords: {keywords}"))
                    .report(filter);
            }
        }

        // ignore overlapping keywords when checking order
        let unsorted_keywords = keywords_map
            .values()
            .filter_map(|x| x.first())
            .collect::<Vec<_>>();
        let sorted_keywords = unsorted_keywords.iter().sorted().collect::<Vec<_>>();
        let sorted_diff = unsorted_keywords
            .iter()
            .zip(sorted_keywords)
            .find(|(a, b)| a != b);
        if let Some((unsorted, sorted)) = sorted_diff {
            KeywordsUnsorted
                .version(pkg)
                .message(format!("unsorted KEYWORD: {unsorted} (sorted: {sorted})"))
                .report(filter);
        }
    }

    fn finish(&self, repo: &EbuildRepo, filter: &mut ReportFilter) {
        if filter.finalize(ArchesUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            ArchesUnused.repo(repo).message(unused).report(filter);
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
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/**/reports.json");
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
