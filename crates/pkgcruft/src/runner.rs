use std::time::Instant;

use indexmap::IndexMap;
use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Scope;
use tracing::{debug, warn};

use crate::check::*;
use crate::iter::ReportFilter;
use crate::scan::ScannerRun;
use crate::source::*;

/// Target to run checks against.
#[derive(Debug)]
pub(super) enum Target {
    Cpv(Cpv),
    Cpn(Cpn),
    Repo,
}

impl Target {
    /// Return the target scope.
    fn scope(&self) -> Scope {
        match self {
            Self::Cpv(_) => Scope::Version,
            Self::Cpn(_) => Scope::Package,
            Self::Repo => Scope::Repo,
        }
    }
}

/// Check runner for synchronous checks.
pub(super) struct SyncCheckRunner {
    runners: IndexMap<SourceKind, CheckRunner>,
}

impl SyncCheckRunner {
    pub(super) fn new(run: &ScannerRun, filter: &ReportFilter) -> Self {
        let mut runners = IndexMap::new();

        for check in &run.checks {
            for source in check
                .sources()
                .iter()
                .filter(|x| x.scope() <= run.scope)
                .copied()
            {
                runners
                    .entry(source)
                    .or_insert_with(|| CheckRunner::new(source, run))
                    .add_check(*check, filter)
            }
        }

        Self { runners }
    }

    /// Return an iterator of all the runner's checks.
    pub(super) fn checks(&self) -> impl Iterator<Item = Check> + '_ {
        self.runners.values().flat_map(|r| r.iter()).unique()
    }

    /// Run all checks that work with the target's scope.
    pub(super) fn run_checks(&self, target: &Target, filter: &ReportFilter) {
        for (source, runner) in self
            .runners
            .iter()
            .filter(|(source, _)| target.scope() >= source.scope())
        {
            runner.run_checks(target, filter, source);
        }
    }

    /// Run a check.
    pub(super) fn run_check(&self, check: &Check, target: &Target, filter: &ReportFilter) {
        for runner in check
            .sources()
            .iter()
            .filter(|x| target.scope() >= x.scope())
            .filter_map(|x| self.runners.get(x))
        {
            runner.run_check(check, target, filter);
        }
    }

    /// Run finalization for a target.
    pub(super) fn finish_target(&self, check: &Check, target: &Target, filter: &ReportFilter) {
        for runner in check
            .sources()
            .iter()
            .filter(|x| target.scope() == x.scope())
            .filter_map(|x| self.runners.get(x))
        {
            runner.finish_target(check, target, filter);
        }
    }

    /// Run finalization for a check.
    pub(super) fn finish_check(&self, check: Check, filter: &ReportFilter) {
        for runner in check.sources().iter().filter_map(|x| self.runners.get(x)) {
            runner.finish_check(&check, filter);
        }
    }
}

/// Generic check runners.
enum CheckRunner {
    EbuildPkg(EbuildPkgCheckRunner),
    EbuildRawPkg(EbuildRawPkgCheckRunner),
    Cpn(CpnCheckRunner),
    Cpv(CpvCheckRunner),
    Repo(RepoCheckRunner),
}

impl CheckRunner {
    fn new(source: SourceKind, run: &ScannerRun) -> Self {
        match source {
            SourceKind::EbuildPkg => Self::EbuildPkg(EbuildPkgCheckRunner::new(run)),
            SourceKind::EbuildRawPkg => Self::EbuildRawPkg(EbuildRawPkgCheckRunner::new(run)),
            SourceKind::Cpn => Self::Cpn(CpnCheckRunner::new(run)),
            SourceKind::Cpv => Self::Cpv(CpvCheckRunner::new(run)),
            SourceKind::Repo => Self::Repo(RepoCheckRunner::new(run)),
        }
    }

    /// Return the iterator of registered checks.
    fn iter(&self) -> Box<dyn Iterator<Item = Check> + '_> {
        match self {
            Self::EbuildPkg(r) => Box::new(r.iter()),
            Self::EbuildRawPkg(r) => Box::new(r.iter()),
            Self::Cpn(r) => Box::new(r.iter()),
            Self::Cpv(r) => Box::new(r.iter()),
            Self::Repo(r) => Box::new(r.iter()),
        }
    }

    /// Add a check to the runner.
    fn add_check(&mut self, check: Check, filter: &ReportFilter) {
        match self {
            Self::EbuildPkg(r) => r.add_check(check, filter),
            Self::EbuildRawPkg(r) => r.add_check(check, filter),
            Self::Cpn(r) => r.add_check(check, filter),
            Self::Cpv(r) => r.add_check(check, filter),
            Self::Repo(r) => r.add_check(check, filter),
        }
    }

    /// Run all check runners in order of priority.
    fn run_checks(&self, target: &Target, filter: &ReportFilter, source: &SourceKind) {
        match (self, target) {
            (Self::EbuildPkg(r), Target::Cpn(cpn)) => r.run_checks(cpn, filter),
            (Self::EbuildRawPkg(r), Target::Cpn(cpn)) => r.run_checks(cpn, filter),
            (Self::Cpn(r), Target::Cpn(cpn)) => r.run_checks(cpn, filter),
            (Self::Cpv(r), Target::Cpn(cpn)) => r.run_checks(cpn, filter),
            _ => unreachable!("incompatible target {target:?} for source: {source}"),
        }
    }

    /// Run a check.
    fn run_check(&self, check: &Check, target: &Target, filter: &ReportFilter) {
        match (self, target) {
            (Self::EbuildPkg(r), Target::Cpv(cpv)) => r.run_check(check, cpv, filter),
            (Self::EbuildPkg(r), Target::Cpn(cpn)) => r.run_pkg_set(check, cpn, filter),
            (Self::EbuildRawPkg(r), Target::Cpv(cpv)) => r.run_check(check, cpv, filter),
            (Self::EbuildRawPkg(r), Target::Cpn(cpn)) => r.run_pkg_set(check, cpn, filter),
            (Self::Cpn(r), Target::Cpn(cpn)) => r.run_check(check, cpn, filter),
            (Self::Cpv(r), Target::Cpv(cpv)) => r.run_check(check, cpv, filter),
            (Self::Repo(r), Target::Repo) => r.run_check(check, filter),
            _ => unreachable!("incompatible target {target:?} for check: {check}"),
        }
    }

    /// Run finalization for a target.
    fn finish_target(&self, check: &Check, target: &Target, filter: &ReportFilter) {
        match (self, target) {
            (Self::Cpn(r), Target::Cpn(cpn)) => r.finish_target(check, cpn, filter),
            (Self::Cpv(r), Target::Cpv(cpv)) => r.finish_target(check, cpv, filter),
            _ => unreachable!("incompatible target {target:?} for check: {check}"),
        }
    }

    /// Run finalization for a check.
    fn finish_check(&self, check: &Check, filter: &ReportFilter) {
        match self {
            Self::EbuildPkg(r) => r.finish_check(check, filter),
            Self::EbuildRawPkg(r) => r.finish_check(check, filter),
            Self::Repo(r) => r.finish_check(check, filter),
            // TODO: revert to panicking when finish_check() filters runners
            _ => (),
        }
    }
}

/// Create generic package check runners.
macro_rules! make_pkg_check_runner {
    ($pkg_check_runner:ident, $pkg_runner:ty, $pkg_set_runner:ty, $source:ty, $pkg:ty) => {
        /// Check runner for package checks.
        struct $pkg_check_runner {
            pkg_checks: IndexMap<Check, $pkg_runner>,
            pkg_set_checks: IndexMap<Check, $pkg_set_runner>,
            source: $source,
            cache: PkgCache<$pkg>,
            repo: EbuildRepo,
        }

        impl $pkg_check_runner {
            fn new(run: &ScannerRun) -> Self {
                let source = <$source>::new(run);
                let cache = PkgCache::new(&source, run);

                Self {
                    pkg_checks: Default::default(),
                    pkg_set_checks: Default::default(),
                    source,
                    cache,
                    repo: run.repo.clone(),
                }
            }

            /// Add a check to the runner.
            fn add_check(&mut self, check: Check, filter: &ReportFilter) {
                if check.scope() == Scope::Version {
                    self.pkg_checks
                        .insert(check, check.to_runner(&self.repo, filter));
                } else {
                    self.pkg_set_checks
                        .insert(check, check.to_runner(&self.repo, filter));
                }
            }

            /// Return the iterator of registered checks.
            fn iter(&self) -> impl Iterator<Item = Check> + '_ {
                self.pkg_checks
                    .keys()
                    .chain(self.pkg_set_checks.keys())
                    .cloned()
            }

            /// Run all checks for a [`Cpn`].
            fn run_checks(&self, cpn: &Cpn, filter: &ReportFilter) {
                let source = &self.source;
                let mut pkgs = Ok(vec![]);

                for result in source.iter_restrict(cpn) {
                    match result {
                        Ok(pkg) => {
                            for (check, runner) in &self.pkg_checks {
                                let now = Instant::now();
                                runner.run(&pkg, filter);
                                debug!("{check}: {pkg}: {:?}", now.elapsed());
                            }

                            if !self.pkg_set_checks.is_empty() {
                                if let Ok(pkgs) = pkgs.as_mut() {
                                    pkgs.push(pkg);
                                }
                            }
                        }
                        Err(e) => pkgs = Err(e),
                    }
                }

                match &pkgs {
                    Ok(pkgs) => {
                        if !pkgs.is_empty() {
                            for (check, runner) in &self.pkg_set_checks {
                                let now = Instant::now();
                                runner.run(cpn, pkgs, filter);
                                debug!("{check}: {cpn}: {:?}", now.elapsed());
                            }
                        }
                    }
                    Err(e) => warn!("skipping {source} set checks due to {e}"),
                }
            }

            /// Run a check for a [`Cpn`].
            fn run_pkg_set(&self, check: &Check, cpn: &Cpn, filter: &ReportFilter) {
                match self.cache.get_pkgs() {
                    Ok(pkgs) => {
                        if !pkgs.is_empty() {
                            let runner = self
                                .pkg_set_checks
                                .get(check)
                                .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                            let now = Instant::now();
                            runner.run(cpn, pkgs, filter);
                            debug!("{check}: {cpn}: {:?}", now.elapsed());
                        }
                    }
                    Err(e) => warn!("{check}: skipping due to {e}"),
                }
            }

            /// Run a check for a [`Cpv`].
            fn run_check(&self, check: &Check, cpv: &Cpv, filter: &ReportFilter) {
                match self.cache.get_pkg(cpv) {
                    Some(Ok(pkg)) => {
                        let runner = self
                            .pkg_checks
                            .get(check)
                            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                        let now = Instant::now();
                        runner.run(pkg, filter);
                        debug!("{check}: {cpv}: {:?}", now.elapsed());
                    }
                    Some(Err(e)) => warn!("{check}: skipping due to {e}"),
                    None => warn!("{check}: skipping due to filtered pkg: {cpv}"),
                }
            }

            /// Finish a check.
            fn finish_check(&self, check: &Check, filter: &ReportFilter) {
                let now = Instant::now();
                if check.scope() == Scope::Version {
                    let runner = self
                        .pkg_checks
                        .get(check)
                        .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                    runner.finish_check(&self.repo, filter);
                } else {
                    let runner = self
                        .pkg_set_checks
                        .get(check)
                        .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                    runner.finish_check(&self.repo, filter);
                }
                debug!("{check}: finish: {:?}", now.elapsed());
            }
        }
    };
}

// Check runner for ebuild package checks.
make_pkg_check_runner!(
    EbuildPkgCheckRunner,
    EbuildPkgRunner,
    EbuildPkgSetRunner,
    EbuildPkgSource,
    EbuildPkg
);

// Check runner for raw ebuild package checks.
make_pkg_check_runner!(
    EbuildRawPkgCheckRunner,
    EbuildRawPkgRunner,
    EbuildRawPkgSetRunner,
    EbuildRawPkgSource,
    EbuildRawPkg
);

/// Check runner for [`Cpn`] objects.
struct CpnCheckRunner {
    checks: IndexMap<Check, CpnRunner>,
    repo: EbuildRepo,
}

impl CpnCheckRunner {
    fn new(run: &ScannerRun) -> Self {
        Self {
            checks: Default::default(),
            repo: run.repo.clone(),
        }
    }

    /// Add a check to the runner.
    fn add_check(&mut self, check: Check, filter: &ReportFilter) {
        self.checks
            .insert(check, check.to_runner(&self.repo, filter));
    }

    /// Return the iterator of registered checks.
    fn iter(&self) -> impl Iterator<Item = Check> + '_ {
        self.checks.keys().cloned()
    }

    /// Run all checks for a [`Cpn`].
    fn run_checks(&self, cpn: &Cpn, filter: &ReportFilter) {
        for (check, runner) in &self.checks {
            let now = Instant::now();
            runner.run(cpn, filter);
            debug!("{check}: {cpn}: {:?}", now.elapsed());

            // run finalize methods for a target
            if check.finish_target() {
                self.finish_target(check, cpn, filter);
            }
        }
    }

    /// Run a check for a [`Cpn`].
    fn run_check(&self, check: &Check, cpn: &Cpn, filter: &ReportFilter) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(cpn, filter);
        debug!("{check}: {cpn}: {:?}", now.elapsed());
    }

    /// Finish a target.
    fn finish_target(&self, check: &Check, cpn: &Cpn, filter: &ReportFilter) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_target(cpn, filter);
        debug!("{check}: {cpn}: finish target: {:?}", now.elapsed());
    }
}

/// Check runner for [`Cpv`] objects.
struct CpvCheckRunner {
    checks: IndexMap<Check, CpvRunner>,
    repo: EbuildRepo,
}

impl CpvCheckRunner {
    fn new(run: &ScannerRun) -> Self {
        Self {
            checks: Default::default(),
            repo: run.repo.clone(),
        }
    }

    /// Add a check to the runner.
    fn add_check(&mut self, check: Check, filter: &ReportFilter) {
        self.checks
            .insert(check, check.to_runner(&self.repo, filter));
    }

    /// Return the iterator of registered checks.
    fn iter(&self) -> impl Iterator<Item = Check> + '_ {
        self.checks.keys().cloned()
    }

    /// Run all checks for a [`Cpn`].
    fn run_checks(&self, cpn: &Cpn, filter: &ReportFilter) {
        for cpv in self.repo.iter_cpv_restrict(cpn) {
            for (check, runner) in &self.checks {
                let now = Instant::now();
                runner.run(&cpv, filter);
                debug!("{check}: {cpv}: {:?}", now.elapsed());

                // run finalize methods for a target
                if check.finish_target() {
                    self.finish_target(check, &cpv, filter);
                }
            }
        }
    }

    /// Run a check for a [`Cpv`].
    fn run_check(&self, check: &Check, cpv: &Cpv, filter: &ReportFilter) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(cpv, filter);
        debug!("{check}: {cpv}: {:?}", now.elapsed());
    }

    /// Finish a target.
    fn finish_target(&self, check: &Check, cpv: &Cpv, filter: &ReportFilter) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_target(cpv, filter);
        debug!("{check}: {cpv}: finish target: {:?}", now.elapsed());
    }
}

/// Check runner for Repo objects.
struct RepoCheckRunner {
    checks: IndexMap<Check, RepoRunner>,
    repo: EbuildRepo,
}

impl RepoCheckRunner {
    fn new(run: &ScannerRun) -> Self {
        Self {
            checks: Default::default(),
            repo: run.repo.clone(),
        }
    }

    /// Add a check to the runner.
    fn add_check(&mut self, check: Check, filter: &ReportFilter) {
        self.checks
            .insert(check, check.to_runner(&self.repo, filter));
    }

    /// Return the iterator of registered checks.
    fn iter(&self) -> impl Iterator<Item = Check> + '_ {
        self.checks.keys().cloned()
    }

    /// Run a check.
    fn run_check(&self, check: &Check, filter: &ReportFilter) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(&self.repo, filter);
        debug!("{check}: {} {:?}", self.repo, now.elapsed());
    }

    /// Finish a check.
    fn finish_check(&self, check: &Check, filter: &ReportFilter) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_check(&self.repo, filter);
        debug!("{check}: finish: {:?}", now.elapsed());
    }
}
