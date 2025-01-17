use std::sync::Arc;
use std::time::Instant;

use enum_as_inner::EnumAsInner;
use indexmap::IndexMap;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Scope;
use tracing::{debug, warn};

use crate::check::*;
use crate::scan::ScannerRun;
use crate::source::*;

/// Target to run checks against.
#[derive(EnumAsInner, Debug, Eq, PartialEq, Hash)]
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

impl From<Cpn> for Target {
    fn from(value: Cpn) -> Self {
        Self::Cpn(value)
    }
}

impl From<Cpv> for Target {
    fn from(value: Cpv) -> Self {
        Self::Cpv(value)
    }
}

#[allow(unused_variables)]
pub(super) trait CheckRunner {
    /// Add a check to the runner.
    fn add_check(&mut self, check: Check, run: &ScannerRun);

    /// Run all checks for a [`Cpn`];
    fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {}

    /// Run a check.
    fn run_check(&self, check: &Check, target: &Target, run: &ScannerRun);

    /// Run finalization for a target.
    fn finish_target(&self, check: &Check, target: &Target, run: &ScannerRun) {}

    /// Run finalization for a check.
    fn finish_check(&self, check: &Check, run: &ScannerRun) {}
}

/// Check runner for synchronous checks.
#[derive(Default)]
pub(super) struct SyncCheckRunner {
    runners: IndexMap<SourceKind, Box<dyn CheckRunner + Send + Sync>>,
}

impl SyncCheckRunner {
    pub(super) fn new(run: &Arc<ScannerRun>) -> Self {
        let mut runner = Self::default();

        for check in &run.checks {
            runner.add_check(*check, run);
        }

        runner
    }
}

impl CheckRunner for SyncCheckRunner {
    fn add_check(&mut self, check: Check, run: &ScannerRun) {
        for source in check
            .sources()
            .iter()
            .filter(|x| x.scope() <= run.scope)
            .copied()
        {
            self.runners
                .entry(source)
                .or_insert_with(|| match source {
                    SourceKind::EbuildPkg => Box::new(EbuildPkgCheckRunner::new(run)),
                    SourceKind::EbuildRawPkg => Box::new(EbuildRawPkgCheckRunner::new(run)),
                    SourceKind::Cpn => Box::new(CpnCheckRunner::new()),
                    SourceKind::Cpv => Box::new(CpvCheckRunner::new(run)),
                    SourceKind::Repo => Box::new(RepoCheckRunner::new(run)),
                })
                .add_check(check, run)
        }
    }

    fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {
        for (_, runner) in self
            .runners
            .iter()
            .filter(|(source, _)| Scope::Package >= source.scope())
        {
            runner.run_checks(cpn, run);
        }
    }

    fn run_check(&self, check: &Check, target: &Target, run: &ScannerRun) {
        for runner in check
            .sources()
            .iter()
            .filter(|x| target.scope() >= x.scope())
            .filter_map(|x| self.runners.get(x))
        {
            runner.run_check(check, target, run);
        }
    }

    fn finish_target(&self, check: &Check, target: &Target, run: &ScannerRun) {
        for runner in check
            .sources()
            .iter()
            .filter(|x| target.scope() == x.scope())
            .filter_map(|x| self.runners.get(x))
        {
            runner.finish_target(check, target, run);
        }
    }

    fn finish_check(&self, check: &Check, run: &ScannerRun) {
        for runner in check.sources().iter().filter_map(|x| self.runners.get(x)) {
            runner.finish_check(check, run);
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

            /// Run a check for a [`Cpv`].
            fn run_pkg(&self, check: &Check, cpv: &Cpv, run: &ScannerRun) {
                match self.cache.get_pkg(cpv) {
                    Some(Ok(pkg)) => {
                        let runner = self
                            .pkg_checks
                            .get(check)
                            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                        let now = Instant::now();
                        runner.run(pkg, run);
                        debug!("{check}: {cpv}: {:?}", now.elapsed());
                    }
                    Some(Err(e)) => warn!("{check}: skipping due to {e}"),
                    None => warn!("{check}: skipping due to filtered pkg: {cpv}"),
                }
            }

            /// Run a check for a [`Cpn`].
            fn run_pkg_set(&self, check: &Check, cpn: &Cpn, run: &ScannerRun) {
                match self.cache.get_pkgs() {
                    Ok(pkgs) => {
                        if !pkgs.is_empty() {
                            let runner = self
                                .pkg_set_checks
                                .get(check)
                                .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                            let now = Instant::now();
                            runner.run(cpn, pkgs, run);
                            debug!("{check}: {cpn}: {:?}", now.elapsed());
                        }
                    }
                    Err(e) => warn!("{check}: skipping due to {e}"),
                }
            }
        }

        impl CheckRunner for $pkg_check_runner {
            fn add_check(&mut self, check: Check, run: &ScannerRun) {
                if check.scope() == Scope::Version {
                    self.pkg_checks.insert(check, check.to_runner(run));
                } else {
                    self.pkg_set_checks.insert(check, check.to_runner(run));
                }
            }

            fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {
                let source = &self.source;
                let mut pkgs = Ok(vec![]);

                for result in source.iter_restrict(cpn) {
                    match result {
                        Ok(pkg) => {
                            for (check, runner) in &self.pkg_checks {
                                let now = Instant::now();
                                runner.run(&pkg, run);
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
                                runner.run(cpn, pkgs, run);
                                debug!("{check}: {cpn}: {:?}", now.elapsed());
                            }
                        }
                    }
                    Err(e) => warn!("skipping {source} set checks due to {e}"),
                }
            }

            fn run_check(&self, check: &Check, target: &Target, run: &ScannerRun) {
                match target {
                    Target::Cpv(cpv) => self.run_pkg(check, cpv, run),
                    Target::Cpn(cpn) => self.run_pkg_set(check, cpn, run),
                    _ => unreachable!("incompatible target {target:?}"),
                }
            }

            fn finish_check(&self, check: &Check, run: &ScannerRun) {
                let now = Instant::now();
                if check.scope() == Scope::Version {
                    let runner = self
                        .pkg_checks
                        .get(check)
                        .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                    runner.finish_check(&self.repo, run);
                } else {
                    let runner = self
                        .pkg_set_checks
                        .get(check)
                        .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                    runner.finish_check(&self.repo, run);
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
#[derive(Default)]
struct CpnCheckRunner {
    checks: IndexMap<Check, CpnRunner>,
}

impl CpnCheckRunner {
    fn new() -> Self {
        Self::default()
    }

    fn finish_cpn(&self, check: &Check, cpn: &Cpn, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_target(cpn, run);
        debug!("{check}: {cpn}: finish target: {:?}", now.elapsed());
    }
}

impl CheckRunner for CpnCheckRunner {
    fn add_check(&mut self, check: Check, run: &ScannerRun) {
        self.checks.insert(check, check.to_runner(run));
    }

    fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {
        for (check, runner) in &self.checks {
            let now = Instant::now();
            runner.run(cpn, run);
            debug!("{check}: {cpn}: {:?}", now.elapsed());

            // run finalize methods for a target
            if check.finish_target() {
                self.finish_cpn(check, cpn, run);
            }
        }
    }

    fn run_check(&self, check: &Check, target: &Target, run: &ScannerRun) {
        let cpn = target
            .as_cpn()
            .unwrap_or_else(|| panic!("invalid target: {target:?}"));
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(cpn, run);
        debug!("{check}: {cpn}: {:?}", now.elapsed());
    }

    fn finish_target(&self, check: &Check, target: &Target, run: &ScannerRun) {
        let cpn = target
            .as_cpn()
            .unwrap_or_else(|| panic!("invalid target: {target:?}"));
        self.finish_cpn(check, cpn, run);
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

    fn finish_cpv(&self, check: &Check, cpv: &Cpv, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_target(cpv, run);
        debug!("{check}: {cpv}: finish target: {:?}", now.elapsed());
    }
}

impl CheckRunner for CpvCheckRunner {
    fn add_check(&mut self, check: Check, run: &ScannerRun) {
        self.checks.insert(check, check.to_runner(run));
    }

    fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {
        for cpv in self.repo.iter_cpv_restrict(cpn) {
            for (check, runner) in &self.checks {
                let now = Instant::now();
                runner.run(&cpv, run);
                debug!("{check}: {cpv}: {:?}", now.elapsed());

                // run finalize methods for a target
                if check.finish_target() {
                    self.finish_cpv(check, &cpv, run);
                }
            }
        }
    }

    fn run_check(&self, check: &Check, target: &Target, run: &ScannerRun) {
        let cpv = target
            .as_cpv()
            .unwrap_or_else(|| panic!("invalid target: {target:?}"));
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(cpv, run);
        debug!("{check}: {cpv}: {:?}", now.elapsed());
    }

    fn finish_target(&self, check: &Check, target: &Target, run: &ScannerRun) {
        let cpv = target
            .as_cpv()
            .unwrap_or_else(|| panic!("invalid target: {target:?}"));
        self.finish_cpv(check, cpv, run);
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
}

impl CheckRunner for RepoCheckRunner {
    fn add_check(&mut self, check: Check, run: &ScannerRun) {
        self.checks.insert(check, check.to_runner(run));
    }

    fn run_check(&self, check: &Check, _target: &Target, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(&self.repo, run);
        debug!("{check}: {} {:?}", self.repo, now.elapsed());
    }

    fn finish_check(&self, check: &Check, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_check(&self.repo, run);
        debug!("{check}: finish: {:?}", now.elapsed());
    }
}
