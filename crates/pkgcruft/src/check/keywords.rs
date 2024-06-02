use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::KeywordStatus::Stable;
use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::{
    Report,
    ReportKind::{EapiUnstable, KeywordsOverlapping, KeywordsUnsorted},
};
use crate::scope::Scope;
use crate::source::SourceKind;

pub(super) static CHECK: super::CheckInfo = super::CheckInfo {
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[EapiUnstable, KeywordsOverlapping, KeywordsUnsorted],
    context: &[],
    priority: 0,
};

#[derive(Debug)]
pub(crate) struct Check<'a> {
    repo: &'a Repo,
}

impl<'a> Check<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { repo }
    }
}

impl<'a> super::CheckRun<&Pkg<'a>> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkg: &Pkg<'a>, mut report: F) {
        let keywords_map = pkg
            .keywords()
            .iter()
            .map(|k| (k.arch(), k))
            .collect::<OrderedMap<_, OrderedSet<_>>>();
        let overlapping = keywords_map
            .values()
            .filter(|keywords| keywords.len() > 1)
            .collect::<Vec<_>>();

        if !overlapping.is_empty() {
            let message = overlapping
                .into_iter()
                .map(|keywords| format!("({})", keywords.iter().sorted().join(", ")))
                .join(", ");
            report(KeywordsOverlapping.version(pkg, message));
        }

        if self
            .repo
            .metadata
            .config
            .eapis_testing
            .contains(pkg.eapi().as_ref())
        {
            let keywords = pkg
                .keywords()
                .iter()
                .filter(|k| k.status() == Stable)
                .sorted()
                .collect::<Vec<_>>();
            if !keywords.is_empty() {
                let message = format!(
                    "unstable EAPI {} with stable keywords: {}",
                    pkg.eapi(),
                    keywords.into_iter().join(" ")
                );
                report(EapiUnstable.version(pkg, message));
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
            let message = format!("unsorted KEYWORD: {unsorted} (sorted: {sorted})");
            report(KeywordsUnsorted.version(pkg, message));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::check::CheckKind::Keywords;
    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(Keywords);
        let scanner = Scanner::new().jobs(1).checks([Keywords]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
