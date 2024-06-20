use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use camino::Utf8Path;
use indexmap::IndexSet;
use once_cell::sync::Lazy;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild;
use pkgcraft::repo::{ebuild::Repo, Repository};
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator, VariantNames};

use crate::bash::Tree;
use crate::report::ReportKind;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;
use crate::Error;

mod dependency;
mod dependency_slot_missing;
mod duplicates;
mod eapi_stale;
mod eapi_status;
mod header;
mod keywords;
mod keywords_dropped;
mod license;
mod live;
mod metadata;
mod overlay;
mod python_update;
mod restrict_test_missing;
mod ruby_update;
mod unstable_only;
mod use_local;
mod variable_order;
mod whitespace;

/// Check variants.
#[derive(
    AsRefStr,
    Display,
    EnumIter,
    EnumString,
    VariantNames,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Copy,
    Clone,
)]
pub enum CheckKind {
    Dependency,
    DependencySlotMissing,
    Duplicates,
    EapiStale,
    EapiStatus,
    Header,
    Keywords,
    KeywordsDropped,
    License,
    Live,
    Metadata,
    Overlay,
    PythonUpdate,
    RestrictTestMissing,
    RubyUpdate,
    UnstableOnly,
    UseLocal,
    VariableOrder,
    Whitespace,
}

impl From<CheckKind> for Check {
    fn from(value: CheckKind) -> Self {
        use CheckKind::*;
        match value {
            Dependency => dependency::CHECK,
            DependencySlotMissing => dependency_slot_missing::CHECK,
            Duplicates => duplicates::CHECK,
            EapiStale => eapi_stale::CHECK,
            EapiStatus => eapi_status::CHECK,
            Header => header::CHECK,
            Keywords => keywords::CHECK,
            KeywordsDropped => keywords_dropped::CHECK,
            License => license::CHECK,
            Live => live::CHECK,
            Metadata => metadata::CHECK,
            Overlay => overlay::CHECK,
            PythonUpdate => python_update::CHECK,
            RestrictTestMissing => restrict_test_missing::CHECK,
            RubyUpdate => ruby_update::CHECK,
            UnstableOnly => unstable_only::CHECK,
            UseLocal => use_local::CHECK,
            VariableOrder => variable_order::CHECK,
            Whitespace => whitespace::CHECK,
        }
    }
}

/// Check contexts.
#[derive(PartialEq, Eq, Hash, Copy, Clone)]
enum CheckContext {
    /// Check only runs by default in the gentoo repo.
    Gentoo,

    /// Check only runs in repos inheriting from the gentoo repo.
    GentooInherited,

    /// Check isn't enabled by default.
    Optional,

    /// Check only runs in overlay repos.
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

/// Run a check against an unversioned package.
pub(crate) trait UnversionedPkgCheck: RegisterCheck {
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter);
}
pub(crate) type UnversionedPkgRunner = Box<dyn UnversionedPkgCheck + Send + Sync>;

/// Run a check against a given ebuild package version.
pub(crate) trait VersionCheck: RegisterCheck {
    fn run(&self, pkg: &ebuild::Pkg, filter: &mut ReportFilter);
}
pub(crate) type VersionRunner = Box<dyn VersionCheck + Send + Sync>;

/// Run a check against a given ebuild package set.
pub(crate) trait PackageSetCheck: RegisterCheck {
    fn run(&self, cpn: &Cpn, pkgs: &[ebuild::Pkg], filter: &mut ReportFilter);
}
pub(crate) type PackageSetRunner = Box<dyn PackageSetCheck + Send + Sync>;

/// Run a check against a given raw ebuild package version and lazily parsed bash tree.
pub(crate) trait EbuildRawPkgCheck: RegisterCheck {
    fn run(&self, pkg: &ebuild::raw::Pkg, tree: &Tree, filter: &mut ReportFilter);
}
pub(crate) type EbuildRawPkgRunner = Box<dyn EbuildRawPkgCheck + Send + Sync>;

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
        CheckKind::iter().map(Into::into)
    }

    /// Return an iterator of all checks enabled by default.
    pub fn iter_default() -> impl Iterator<Item = Check> {
        Check::iter().filter(|x| !x.context.contains(&CheckContext::Optional))
    }

    /// Return an iterator of checks that generate a given report.
    pub fn iter_report(report: &ReportKind) -> impl Iterator<Item = Check> {
        REPORT_CHECKS
            .get(report)
            .unwrap_or_else(|| unreachable!("no checks for report: {report}"))
            .iter()
            .copied()
    }

    /// Return an iterator of checks that use a given source.
    pub fn iter_source(source: &SourceKind) -> impl Iterator<Item = Check> {
        SOURCE_CHECKS
            .get(source)
            .unwrap_or_else(|| unreachable!("no checks for source: {source}"))
            .iter()
            .copied()
    }

    /// Determine if a check is enabled for a scanning run.
    pub(crate) fn enabled(&self, repo: &Repo, selected: &IndexSet<Self>) -> bool {
        self.context.iter().all(|x| match x {
            CheckContext::Gentoo => repo.name() == "gentoo",
            CheckContext::GentooInherited => repo.trees().any(|x| x.name() == "gentoo"),
            CheckContext::Optional => selected.contains(self),
            CheckContext::Overlay => repo.masters().next().is_some(),
        })
    }
}

/// Create a check runner from a given check.
pub(crate) trait ToRunner<T> {
    fn to_runner(&self, repo: &'static Repo) -> T;
}

impl ToRunner<VersionRunner> for Check {
    fn to_runner(&self, repo: &'static Repo) -> VersionRunner {
        match &self.kind {
            CheckKind::Dependency => Box::new(dependency::create(repo)),
            CheckKind::DependencySlotMissing => Box::new(dependency_slot_missing::create(repo)),
            CheckKind::Keywords => Box::new(keywords::create(repo)),
            CheckKind::License => Box::new(license::create(repo)),
            CheckKind::Overlay => Box::new(overlay::create(repo)),
            CheckKind::PythonUpdate => Box::new(python_update::create(repo)),
            CheckKind::RubyUpdate => Box::new(ruby_update::create(repo)),
            CheckKind::RestrictTestMissing => Box::new(restrict_test_missing::create()),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<PackageSetRunner> for Check {
    fn to_runner(&self, repo: &'static Repo) -> PackageSetRunner {
        match &self.kind {
            CheckKind::EapiStale => Box::new(eapi_stale::create()),
            CheckKind::KeywordsDropped => Box::new(keywords_dropped::create(repo)),
            CheckKind::Live => Box::new(live::create()),
            CheckKind::UnstableOnly => Box::new(unstable_only::create(repo)),
            CheckKind::UseLocal => Box::new(use_local::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<EbuildRawPkgRunner> for Check {
    fn to_runner(&self, repo: &'static Repo) -> EbuildRawPkgRunner {
        match &self.kind {
            CheckKind::EapiStatus => Box::new(eapi_status::create(repo)),
            CheckKind::Header => Box::new(header::create()),
            CheckKind::Metadata => Box::new(metadata::create()),
            CheckKind::VariableOrder => Box::new(variable_order::create()),
            CheckKind::Whitespace => Box::new(whitespace::create()),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<UnversionedPkgRunner> for Check {
    fn to_runner(&self, repo: &'static Repo) -> UnversionedPkgRunner {
        match &self.kind {
            CheckKind::Duplicates => Box::new(duplicates::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl fmt::Debug for Check {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

        Ok(kind.into())
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
            .then_with(|| self.kind.cmp(&other.kind))
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

/// The mapping of all report variants to the checks that can generate them.
static REPORT_CHECKS: Lazy<OrderedMap<ReportKind, OrderedSet<Check>>> = Lazy::new(|| {
    Check::iter()
        .flat_map(|c| c.reports.iter().copied().map(move |r| (r, c)))
        .collect()
});

/// The mapping of all source variants to the checks that use them.
static SOURCE_CHECKS: Lazy<OrderedMap<SourceKind, OrderedSet<Check>>> =
    Lazy::new(|| Check::iter().map(|c| (c.source, c)).collect());

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use pretty_assertions::assert_eq;
    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn kind() {
        // verify CheckKind are kept in lexical order
        let kinds: Vec<_> = CheckKind::iter().collect();
        let ordered: Vec<_> = CheckKind::iter().map(|x| x.to_string()).sorted().collect();
        let ordered: Vec<_> = ordered.iter().map(|s| s.parse().unwrap()).collect();
        assert_eq!(&kinds, &ordered);

        // verify all CheckKind variants map to implemented checks
        let checks: Vec<_> = Check::iter().map(|x| x.kind).collect();
        assert_eq!(&kinds, &checks);
    }

    #[test]
    fn report() {
        // verify all reports variants have at least one check
        for kind in ReportKind::iter() {
            assert!(REPORT_CHECKS.get(&kind).is_some(), "no checks for report: {kind}");
        }
    }

    #[test]
    fn source() {
        // verify all source variants have at least one check
        for kind in SourceKind::iter() {
            assert!(SOURCE_CHECKS.get(&kind).is_some(), "no checks for source: {kind}");
        }
    }
}
