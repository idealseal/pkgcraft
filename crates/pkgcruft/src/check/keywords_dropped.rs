use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::keyword::KeywordStatus::Disabled;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::Scope;

use crate::iter::ReportFilter;
use crate::report::ReportKind::KeywordsDropped;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::KeywordsDropped,
    scope: Scope::Package,
    source: SourceKind::EbuildPkg,
    reports: &[KeywordsDropped],
    context: &[],
};

pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgSetCheck {
    Check { repo: repo.clone() }
}

struct Check {
    repo: EbuildRepo,
}

impl EbuildPkgSetCheck for Check {
    fn run(&self, _cpn: &Cpn, pkgs: &[EbuildPkg], filter: &mut ReportFilter) {
        let mut seen = HashSet::new();
        let mut previous = HashSet::new();
        let mut changes = HashMap::<_, _>::new();

        for pkg in pkgs {
            // skip packages without keywords
            if pkg.keywords().is_empty() {
                continue;
            }

            let arches = pkg
                .keywords()
                .iter()
                .map(|k| k.arch())
                .collect::<HashSet<_>>();

            // globbed arches override all dropped keywords
            let drops = if arches.contains("*") {
                HashSet::new()
            } else {
                previous
                    .difference(&arches)
                    .chain(seen.difference(&arches))
                    .copied()
                    .collect()
            };

            for arch in drops {
                if self.repo.arches().contains(arch) {
                    changes.insert(arch.clone(), pkg);
                }
            }

            // ignore missing arches on previous versions that were re-enabled
            if !changes.is_empty() {
                let disabled = pkg
                    .keywords()
                    .iter()
                    .filter(|k| k.status() == Disabled)
                    .map(|k| k.arch())
                    .collect::<HashSet<_>>();
                let adds = arches
                    .difference(&previous)
                    .copied()
                    .collect::<HashSet<_>>();
                for arch in adds.difference(&disabled) {
                    changes.remove(*arch);
                }
            }

            seen.extend(arches.clone());
            previous = arches;
        }

        #[allow(clippy::mutable_key_type)]
        // false positive due to ebuild pkg OnceLock usage
        let mut dropped = HashMap::<_, Vec<_>>::new();
        for (arch, pkg) in changes {
            // TODO: report all pkgs with dropped keywords in verbose mode?
            // only report the latest pkg with dropped keywords
            dropped.entry(pkg).or_default().push(arch);
        }

        for (pkg, arches) in dropped {
            KeywordsDropped
                .version(pkg)
                .message(arches.iter().sorted().join(", "))
                .report(filter);
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
