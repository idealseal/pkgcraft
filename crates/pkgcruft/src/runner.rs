use std::time::Instant;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::repo::ebuild::Repo;
use tracing::debug;

use crate::bash::Tree;
use crate::check::*;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::{self, IterRestrict, PkgFilter, SourceKind};

/// Check runner for synchronous checks.
pub(super) struct SyncCheckRunner {
    runners: IndexMap<SourceKind, CheckRunner>,
}

impl SyncCheckRunner {
    pub(super) fn new(
        repo: &'static Repo,
        filters: &IndexSet<PkgFilter>,
        checks: &IndexSet<Check>,
    ) -> Self {
        let mut runners = IndexMap::new();

        // filter checks
        let enabled = checks
            .iter()
            .filter(|c| {
                if !filters.is_empty() && c.scope != Scope::Version {
                    debug!("{c}: disabled due to package filtering");
                    false
                } else {
                    true
                }
            })
            // TODO: replace checks parameter with selected checks once #194 is implemented
            .filter(|c| c.enabled(repo, checks))
            .copied()
            // sort checks by priority so they run in the correct order
            .sorted();

        for check in enabled {
            runners
                .entry(check.source)
                .or_insert_with(|| CheckRunner::new(check.source, repo, filters.clone()))
                .add_check(check);
        }

        Self { runners }
    }

    /// Run all check runners in order of priority.
    pub(super) fn run(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        for runner in self.runners.values() {
            runner.run(cpn, filter);
        }
    }
}

/// Generic check runners.
// TODO: remove the lint ignore once more variants are added
#[allow(clippy::enum_variant_names)]
enum CheckRunner {
    EbuildPkg(EbuildPkgCheckRunner),
    EbuildRawPkg(EbuildRawPkgCheckRunner),
    UnversionedPkg(UnversionedPkgCheckRunner),
}

impl CheckRunner {
    fn new(source: SourceKind, repo: &'static Repo, filters: IndexSet<PkgFilter>) -> Self {
        match source {
            SourceKind::EbuildPkg => Self::EbuildPkg(EbuildPkgCheckRunner::new(repo, filters)),
            SourceKind::EbuildRawPkg => {
                Self::EbuildRawPkg(EbuildRawPkgCheckRunner::new(repo, filters))
            }
            SourceKind::UnversionedPkg => {
                Self::UnversionedPkg(UnversionedPkgCheckRunner::new(repo))
            }
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match self {
            Self::EbuildPkg(r) => r.add_check(check),
            Self::EbuildRawPkg(r) => r.add_check(check),
            Self::UnversionedPkg(r) => r.add_check(check),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        match self {
            Self::EbuildPkg(r) => r.run(cpn, filter),
            Self::EbuildRawPkg(r) => r.run(cpn, filter),
            Self::UnversionedPkg(r) => r.run(cpn, filter),
        }
    }
}

/// Check runner for ebuild package checks.
struct EbuildPkgCheckRunner {
    pkg_checks: Vec<EbuildPkgRunner>,
    pkg_set_checks: Vec<EbuildPkgSetRunner>,
    source: source::EbuildPkg,
    repo: &'static Repo,
}

impl EbuildPkgCheckRunner {
    fn new(repo: &'static Repo, filters: IndexSet<PkgFilter>) -> Self {
        Self {
            pkg_checks: Default::default(),
            pkg_set_checks: Default::default(),
            source: source::EbuildPkg::new(repo, filters),
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match &check.scope {
            Scope::Version => self.pkg_checks.push(check.to_runner(self.repo)),
            Scope::Package => self.pkg_set_checks.push(check.to_runner(self.repo)),
            _ => unreachable!("unsupported check: {check}"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        let mut pkgs = vec![];

        for pkg in self.source.iter_restrict(cpn) {
            for check in &self.pkg_checks {
                let now = Instant::now();
                check.run(&pkg, filter);
                debug!("{check}: {pkg}: {:?}", now.elapsed());
            }

            if !self.pkg_set_checks.is_empty() {
                pkgs.push(pkg);
            }
        }

        if !pkgs.is_empty() {
            for check in &self.pkg_set_checks {
                let now = Instant::now();
                check.run(cpn, &pkgs[..], filter);
                debug!("{check}: {cpn}: {:?}", now.elapsed());
            }
        }
    }
}

/// Check runner for raw ebuild package checks.
struct EbuildRawPkgCheckRunner {
    checks: Vec<EbuildRawPkgRunner>,
    source: source::EbuildRawPkg,
    repo: &'static Repo,
}

impl EbuildRawPkgCheckRunner {
    fn new(repo: &'static Repo, filters: IndexSet<PkgFilter>) -> Self {
        Self {
            checks: Default::default(),
            source: source::EbuildRawPkg::new(repo, filters),
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match &check.scope {
            Scope::Version => self.checks.push(check.to_runner(self.repo)),
            _ => unreachable!("unsupported check: {check}"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        for pkg in self.source.iter_restrict(cpn) {
            let tree = Tree::new(pkg.data().as_bytes());
            for check in &self.checks {
                let now = Instant::now();
                check.run(&pkg, &tree, filter);
                debug!("{check}: {pkg}: {:?}", now.elapsed());
            }
        }
    }
}

/// Check runner for unversioned package checks.
struct UnversionedPkgCheckRunner {
    checks: Vec<UnversionedPkgRunner>,
    repo: &'static Repo,
}

impl UnversionedPkgCheckRunner {
    fn new(repo: &'static Repo) -> Self {
        Self {
            checks: Default::default(),
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match &check.scope {
            Scope::Package => self.checks.push(check.to_runner(self.repo)),
            _ => unreachable!("unsupported check: {check}"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        for check in &self.checks {
            let now = Instant::now();
            check.run(cpn, filter);
            debug!("{check}: {cpn}: {:?}", now.elapsed());
        }
    }
}
