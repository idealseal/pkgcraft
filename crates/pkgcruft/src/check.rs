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
use strum::Display;

use crate::report::ReportKind;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;
use crate::Error;

mod dependency;
mod dependency_slot_missing;
mod eapi_stale;
mod eapi_status;
mod keywords;
mod keywords_dropped;
mod live_only;
mod metadata;
mod restrict_test_missing;
mod unstable_only;
mod use_local;

/// Check contexts.
#[derive(Display, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum CheckContext {
    Gentoo,
    Optional,
    Overlay,
}

impl CheckContext {
    /// Determine if a context is enabled.
    pub(super) fn enabled(&self, repo: &Repo) -> bool {
        match self {
            Self::Gentoo => repo.name() == "gentoo",
            Self::Optional => false,
            Self::Overlay => repo.masters().next().is_some(),
        }
    }
}

/// Check runner variants.
#[derive(Display, Debug)]
pub(crate) enum Runner<'a> {
    Dependency(dependency::Check<'a>),
    DependencySlotMissing(dependency_slot_missing::Check<'a>),
    EapiStale(eapi_stale::Check),
    EapiStatus(eapi_status::Check<'a>),
    Keywords(keywords::Check<'a>),
    KeywordsDropped(keywords_dropped::Check<'a>),
    LiveOnly(live_only::Check),
    Metadata(metadata::Check<'a>),
    RestrictTestMissing(restrict_test_missing::Check),
    UnstableOnly(unstable_only::Check<'a>),
    UseLocal(use_local::Check<'a>),
}

impl<'a> CheckRun<&ebuild::Pkg<'a>> for Runner<'a> {
    fn run(&self, pkg: &ebuild::Pkg<'a>, filter: &mut ReportFilter) {
        match self {
            Self::Dependency(c) => c.run(pkg, filter),
            Self::DependencySlotMissing(c) => c.run(pkg, filter),
            Self::Keywords(c) => c.run(pkg, filter),
            Self::EapiStatus(c) => c.run(pkg, filter),
            Self::RestrictTestMissing(c) => c.run(pkg, filter),
            _ => unreachable!("{self} is not an ebuild check"),
        }
    }
}

impl<'a> CheckRun<&ebuild::raw::Pkg<'a>> for Runner<'a> {
    fn run(&self, pkg: &ebuild::raw::Pkg<'a>, filter: &mut ReportFilter) {
        match self {
            Self::Metadata(c) => c.run(pkg, filter),
            _ => unreachable!("{self} is not a raw ebuild check"),
        }
    }
}

impl<'a> CheckRun<&[ebuild::Pkg<'a>]> for Runner<'a> {
    fn run(&self, pkgs: &[ebuild::Pkg<'a>], filter: &mut ReportFilter) {
        match self {
            Self::EapiStale(c) => c.run(pkgs, filter),
            Self::KeywordsDropped(c) => c.run(pkgs, filter),
            Self::LiveOnly(c) => c.run(pkgs, filter),
            Self::UnstableOnly(c) => c.run(pkgs, filter),
            Self::UseLocal(c) => c.run(pkgs, filter),
            _ => unreachable!("{self} is not an ebuild pkg set check"),
        }
    }
}

/// Run a check for a given item sending back any generated reports.
pub(crate) trait CheckRun<T> {
    fn run(&self, item: T, filter: &mut ReportFilter);
}

/// Registered check.
pub struct Check {
    /// The check identifier.
    pub name: &'static str,

    /// The scope the check runs in.
    pub scope: Scope,

    /// The source of the values the check runs against.
    pub source: SourceKind,

    /// All the potential report variants generated by the check.
    pub reports: &'static [ReportKind],

    /// Check variant contexts.
    pub context: &'static [CheckContext],

    /// The priority of the check for enabling a deterministic running order.
    priority: i64,
}

impl Check {
    /// Return an iterator of all registered checks.
    pub fn iter() -> impl Iterator<Item = &'static Check> {
        CHECKS.iter().copied()
    }

    /// Create a check runner for a given variant.
    #[rustfmt::skip]
    pub(crate) fn create<'a>(&self, repo: &'a Repo) -> Runner<'a> {
        use Runner::*;
        match self.name {
            "Dependency" => Dependency(dependency::Check::new(repo)),
            "DependencySlotMissing" => DependencySlotMissing(dependency_slot_missing::Check::new(repo)),
            "EapiStale" => EapiStale(eapi_stale::Check),
            "EapiStatus" => EapiStatus(eapi_status::Check::new(repo)),
            "Keywords" => Keywords(keywords::Check::new(repo)),
            "KeywordsDropped" => KeywordsDropped(keywords_dropped::Check::new(repo)),
            "LiveOnly" => LiveOnly(live_only::Check),
            "Metadata" => Metadata(metadata::Check::new(repo)),
            "RestrictTestMissing" => RestrictTestMissing(restrict_test_missing::Check::new()),
            "UnstableOnly" => UnstableOnly(unstable_only::Check::new(repo)),
            "UseLocal" => UseLocal(use_local::Check::new(repo)),
            _ => panic!("unknown check: {}", self.name),
        }
    }
}

impl fmt::Debug for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Check {{ {} }}", self.name)
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl FromStr for &'static Check {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        CHECKS
            .get(s)
            .copied()
            .ok_or_else(|| Error::InvalidValue(format!("unknown check: {s}")))
    }
}

impl PartialEq for Check {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Check {}

impl Hash for Check {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Borrow<str> for &Check {
    fn borrow(&self) -> &str {
        self.name
    }
}

impl Ord for Check {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.name.cmp(other.name))
    }
}

impl PartialOrd for Check {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl AsRef<Utf8Path> for Check {
    fn as_ref(&self) -> &Utf8Path {
        Utf8Path::new(self.name)
    }
}

static CHECKS: Lazy<IndexSet<&'static Check>> = Lazy::new(|| {
    [
        &dependency::CHECK,
        &dependency_slot_missing::CHECK,
        &eapi_stale::CHECK,
        &eapi_status::CHECK,
        &keywords::CHECK,
        &keywords_dropped::CHECK,
        &live_only::CHECK,
        &metadata::CHECK,
        &restrict_test_missing::CHECK,
        &unstable_only::CHECK,
        &use_local::CHECK,
    ]
    .into_iter()
    .collect()
});

/// The mapping of all report variants to the checks that can generate them.
pub static REPORT_CHECKS: Lazy<OrderedMap<ReportKind, OrderedSet<&'static Check>>> =
    Lazy::new(|| {
        CHECKS
            .iter()
            .flat_map(|c| c.reports.iter().copied().map(move |r| (r, *c)))
            .collect()
    });

/// The mapping of all source variants to the checks that use them.
pub static SOURCE_CHECKS: Lazy<OrderedMap<SourceKind, OrderedSet<&'static Check>>> =
    Lazy::new(|| CHECKS.iter().map(|c| (c.source, *c)).collect());
