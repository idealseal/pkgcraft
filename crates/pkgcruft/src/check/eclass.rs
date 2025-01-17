use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::{EbuildRepo, Eclass};

use crate::report::ReportKind::EclassUnused;
use crate::scan::ScannerRun;

use super::EbuildPkgCheck;

pub(super) fn create(run: &ScannerRun) -> impl EbuildPkgCheck {
    let unused = if run.enabled(EclassUnused) {
        run.repo.metadata().eclasses().iter().cloned().collect()
    } else {
        Default::default()
    };

    Check { unused }
}

static CHECK: super::Check = super::Check::Eclass;

struct Check {
    unused: DashSet<Eclass>,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, _run: &ScannerRun) {
        for eclass in pkg.inherited() {
            self.unused.remove(eclass);
        }
    }

    fn finish_check(&self, repo: &EbuildRepo, run: &ScannerRun) {
        if run.enabled(EclassUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            EclassUnused.repo(repo).message(unused).report(run);
        }
    }
}
