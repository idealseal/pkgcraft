use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::KeywordStatus::Stable;
use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::ReportKind::{
    EapiUnstable, KeywordsLive, KeywordsOverlapping, KeywordsUnsorted,
};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Keywords,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[EapiUnstable, KeywordsLive, KeywordsOverlapping, KeywordsUnsorted],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        if !pkg.keywords().is_empty() && pkg.live() {
            KeywordsLive
                .version(pkg)
                .message(pkg.keywords().iter().join(", "))
                .report(filter);
        }

        let keywords_map = pkg
            .keywords()
            .iter()
            .map(|k| (k.arch(), k))
            .collect::<OrderedMap<_, OrderedSet<_>>>();

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
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_ordered_eq, TEST_DATA, TEST_DATA_PATCHED};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, []);
    }
}
