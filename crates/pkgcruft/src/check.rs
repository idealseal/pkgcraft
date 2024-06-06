use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use camino::Utf8Path;
use indexmap::IndexSet;
use once_cell::sync::Lazy;
use pkgcraft::pkg::ebuild;
use pkgcraft::repo::{ebuild::Repo, Repository};
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

use crate::report::ReportKind;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;
use crate::Error;

mod dependency;
mod dependency_slot_missing;
mod eapi_stale;
mod eapi_status;
mod header;
mod keywords;
mod keywords_dropped;
mod license;
mod live;
mod metadata;
mod overlay;
mod restrict_test_missing;
mod unstable_only;
mod use_local;
mod whitespace;

/// Check variants.
#[derive(
    AsRefStr, Display, EnumIter, EnumString, VariantNames, Debug, PartialEq, Eq, Hash, Copy, Clone,
)]
pub enum CheckKind {
    Dependency,
    EapiStale,
    EapiStatus,
    Header,
    Keywords,
    KeywordsDropped,
    License,
    Live,
    Overlay,
    Metadata,
    DependencySlotMissing,
    RestrictTestMissing,
    UnstableOnly,
    UseLocal,
    Whitespace,
}

impl From<CheckKind> for Check {
    fn from(value: CheckKind) -> Self {
        CHECKS
            .get(&value)
            .copied()
            .unwrap_or_else(|| panic!("unknown check: {value}"))
    }
}

/// Check contexts.
#[derive(PartialEq, Eq, Hash, Copy, Clone)]
enum CheckContext {
    Gentoo,
    Optional,
    Overlay,
}

pub(crate) trait RegisterCheck: fmt::Display {
    fn check(&self) -> Check;
}

/// Implement various traits for a given check type.
macro_rules! register {
    ($x:ty) => {
        impl $crate::check::RegisterCheck for $x {
            fn check(&self) -> $crate::check::Check {
                CHECK
            }
        }

        impl std::fmt::Display for $x {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{CHECK}")
            }
        }
    };
}
use register;

/// Run a check against a given ebuild package version.
pub(crate) trait VersionCheck: RegisterCheck {
    fn run(&self, pkg: &ebuild::Pkg, filter: &mut ReportFilter);
}
pub(crate) type VersionCheckRunner = Box<dyn VersionCheck + Send + Sync>;

/// Run a check against a given ebuild package set.
pub(crate) trait PackageCheck: RegisterCheck {
    fn run(&self, pkg: &[ebuild::Pkg], filter: &mut ReportFilter);
}
pub(crate) type PackageCheckRunner = Box<dyn PackageCheck + Send + Sync>;

/// Run a check against a given raw ebuild package version.
pub(crate) trait RawVersionCheck: RegisterCheck {
    fn run(&self, pkg: &ebuild::raw::Pkg, filter: &mut ReportFilter);
}
pub(crate) type RawVersionCheckRunner = Box<dyn RawVersionCheck + Send + Sync>;

/// Registered check.
#[derive(Copy, Clone)]
pub struct Check {
    /// The check variant.
    pub(crate) kind: CheckKind,

    /// The scope the check runs in.
    pub scope: Scope,

    /// The source of the values the check runs against.
    pub source: SourceKind,

    /// All the potential report variants generated by the check.
    pub reports: &'static [ReportKind],

    /// Check variant contexts.
    context: &'static [CheckContext],

    /// The priority of the check for enabling a deterministic running order.
    priority: i64,
}

impl Check {
    /// Return the name of the check.
    pub fn name(&self) -> &str {
        self.kind.as_ref()
    }

    /// Return an iterator of all registered checks.
    pub fn iter() -> impl Iterator<Item = Check> {
        CHECKS.iter().copied()
    }

    /// Return an iterator of all checks enabled by default.
    pub fn iter_default() -> impl Iterator<Item = Check> {
        CHECKS
            .iter()
            .filter(|x| !x.context.contains(&CheckContext::Optional))
            .copied()
    }

    /// Return an iterator of checks that generate a given report.
    pub fn iter_report(report: &ReportKind) -> impl Iterator<Item = Check> {
        REPORT_CHECKS
            .get(report)
            .unwrap_or_else(|| panic!("no checks for report: {report}"))
            .iter()
            .copied()
    }

    /// Return an iterator of checks that use a given source.
    pub fn iter_source(source: &SourceKind) -> impl Iterator<Item = Check> {
        SOURCE_CHECKS
            .get(source)
            .unwrap_or_else(|| panic!("no checks for source: {source}"))
            .iter()
            .copied()
    }

    /// Determine if a check is enabled for a scanning run.
    pub(crate) fn enabled(&self, repo: &Repo, selected: &IndexSet<Self>) -> bool {
        self.context.iter().all(|x| match x {
            CheckContext::Gentoo => repo.name() == "gentoo",
            CheckContext::Optional => selected.contains(self),
            CheckContext::Overlay => repo.masters().next().is_some(),
        })
    }

    /// Create an ebuild package version check runner.
    pub(crate) fn version_check(&self, repo: &'static Repo) -> VersionCheckRunner {
        match &self.kind {
            CheckKind::Dependency => Box::new(dependency::create(repo)),
            CheckKind::DependencySlotMissing => Box::new(dependency_slot_missing::create(repo)),
            CheckKind::EapiStatus => Box::new(eapi_status::create(repo)),
            CheckKind::Keywords => Box::new(keywords::create(repo)),
            CheckKind::License => Box::new(license::create(repo)),
            CheckKind::Overlay => Box::new(overlay::create(repo)),
            CheckKind::RestrictTestMissing => Box::new(restrict_test_missing::create()),
            _ => unreachable!("unsupported check: {self}"),
        }
    }

    /// Create an ebuild package set check runner.
    pub(crate) fn package_check(&self, repo: &'static Repo) -> PackageCheckRunner {
        match &self.kind {
            CheckKind::EapiStale => Box::new(eapi_stale::create()),
            CheckKind::KeywordsDropped => Box::new(keywords_dropped::create(repo)),
            CheckKind::Live => Box::new(live::create()),
            CheckKind::UnstableOnly => Box::new(unstable_only::create(repo)),
            CheckKind::UseLocal => Box::new(use_local::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }

    /// Create a raw ebuild package version check runner.
    pub(crate) fn raw_version_check(&self) -> RawVersionCheckRunner {
        match &self.kind {
            CheckKind::Header => Box::new(header::create()),
            CheckKind::Metadata => Box::new(metadata::create()),
            CheckKind::Whitespace => Box::new(whitespace::create()),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl fmt::Debug for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl FromStr for Check {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let kind: CheckKind = s
            .parse()
            .map_err(|_| Error::InvalidValue(format!("unknown check: {s}")))?;
        CHECKS
            .get(&kind)
            .copied()
            .ok_or_else(|| Error::InvalidValue(format!("unknown check: {s}")))
    }
}

impl PartialEq for Check {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Eq for Check {}

impl Hash for Check {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
    }
}

impl Borrow<CheckKind> for Check {
    fn borrow(&self) -> &CheckKind {
        &self.kind
    }
}

impl Ord for Check {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.name().cmp(other.name()))
    }
}

impl PartialOrd for Check {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl AsRef<Utf8Path> for Check {
    fn as_ref(&self) -> &Utf8Path {
        Utf8Path::new(self.name())
    }
}

/// The set of all registered checks.
static CHECKS: Lazy<IndexSet<Check>> = Lazy::new(|| {
    [
        dependency::CHECK,
        dependency_slot_missing::CHECK,
        eapi_stale::CHECK,
        eapi_status::CHECK,
        header::CHECK,
        keywords::CHECK,
        keywords_dropped::CHECK,
        license::CHECK,
        live::CHECK,
        metadata::CHECK,
        overlay::CHECK,
        restrict_test_missing::CHECK,
        unstable_only::CHECK,
        use_local::CHECK,
        whitespace::CHECK,
    ]
    .into_iter()
    .collect()
});

/// The mapping of all report variants to the checks that can generate them.
static REPORT_CHECKS: Lazy<OrderedMap<ReportKind, OrderedSet<Check>>> = Lazy::new(|| {
    CHECKS
        .iter()
        .flat_map(|c| c.reports.iter().copied().map(move |r| (r, *c)))
        .collect()
});

/// The mapping of all source variants to the checks that use them.
static SOURCE_CHECKS: Lazy<OrderedMap<SourceKind, OrderedSet<Check>>> =
    Lazy::new(|| CHECKS.iter().map(|c| (c.source, *c)).collect());
