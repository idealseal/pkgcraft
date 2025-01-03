use std::fmt;
use std::str::FromStr;

use colored::{Color, Colorize};
use indexmap::{IndexMap, IndexSet};
use itertools::{Either, Itertools};
use pkgcraft::dep::Cpv;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::pkg::ebuild::{keyword::KeywordStatus, EbuildPkg, EbuildRawPkg};
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::{self, Restrict, Restriction, Scope};
use pkgcraft::types::OrderedMap;
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator, VariantNames};

use crate::error::Error;

/// All check runner source variants.
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
#[strum(serialize_all = "kebab-case")]
pub enum SourceKind {
    EbuildPkg,
    EbuildRawPkg,
    Cpn,
    Cpv,
    Repo,
}

/// Package filtering variants.
#[derive(AsRefStr, EnumIter, Debug, PartialEq, Eq, Hash, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum PkgFilter {
    /// Filter packages using the latest version only.
    Latest(bool),

    /// Filter packages using the latest version from each slot.
    LatestSlots(bool),

    /// Filter packages based on live status.
    Live(bool),

    /// Filter packages based on global mask status.
    Masked(bool),

    /// Filter packages using a custom restriction.
    Restrict(bool, Restrict),

    /// Filter packages based on stable keyword status.
    Stable(bool),
}

impl PkgFilter {
    /// Apply filter across an iterator of packages.
    fn filter<'a>(
        &'a self,
        iter: Box<dyn Iterator<Item = EbuildPkg> + 'a>,
    ) -> Box<dyn Iterator<Item = EbuildPkg> + 'a> {
        match self {
            Self::Latest(inverted) => {
                let items: Vec<_> = iter.collect();
                let len = items.len();
                if items.is_empty() {
                    Box::new(items.into_iter())
                } else if *inverted {
                    Box::new(items.into_iter().take(len - 1))
                } else {
                    Box::new(items.into_iter().skip(len - 1))
                }
            }
            Self::LatestSlots(inverted) => Box::new(
                iter.map(|pkg| (pkg.slot().to_string(), pkg))
                    .collect::<OrderedMap<_, Vec<_>>>()
                    .into_values()
                    .flat_map(|pkgs| {
                        let len = pkgs.len();
                        if *inverted {
                            Either::Left(pkgs.into_iter().take(len - 1))
                        } else {
                            Either::Right(pkgs.into_iter().skip(len - 1))
                        }
                    }),
            ),
            Self::Live(inverted) => Box::new(iter.filter(move |pkg| inverted ^ pkg.live())),
            Self::Masked(inverted) => {
                Box::new(iter.filter(move |pkg| inverted ^ pkg.masked()))
            }
            Self::Stable(inverted) => {
                let status = if *inverted {
                    KeywordStatus::Unstable
                } else {
                    KeywordStatus::Stable
                };
                Box::new(iter.filter(move |pkg| {
                    !pkg.keywords().is_empty()
                        && pkg.keywords().iter().all(|k| k.status() == status)
                }))
            }
            Self::Restrict(inverted, restrict) => {
                Box::new(iter.filter(move |pkg| inverted ^ restrict.matches(pkg)))
            }
        }
    }
}

impl FromStr for PkgFilter {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let stripped = s.strip_prefix('!');
        let inverted = stripped.is_some();
        match stripped.unwrap_or(s) {
            "latest" => Ok(Self::Latest(inverted)),
            "latest-slots" => Ok(Self::LatestSlots(inverted)),
            "live" => Ok(Self::Live(inverted)),
            "masked" => Ok(Self::Masked(inverted)),
            "stable" => Ok(Self::Stable(inverted)),
            s if s.contains(|c: char| c.is_whitespace()) => {
                Ok(restrict::parse::pkg(s).map(|r| Self::Restrict(inverted, r))?)
            }
            s => {
                // support dep restrictions
                if let Ok(r) = restrict::parse::dep(s) {
                    return Ok(Self::Restrict(inverted, r));
                }

                let possible = Self::iter()
                    .filter(|r| !matches!(r, Self::Restrict(_, _)))
                    .map(|r| r.as_ref().color(Color::Green))
                    .join(", ");
                let message = indoc::formatdoc! {r#"
                    invalid filter: {s}
                      [possible values: {possible}]

                    Dep restrictions are supported, for example the following will scan
                    all packages in the sys-devel category:

                    pkgcruft scan -f 'sys-devel/*'

                    Custom restrictions are supported, for example to target all packages
                    maintained by the python project use the following command:

                    pkgcruft scan -f "maintainers any email == 'python@gentoo.org'""#};
                Err(Error::InvalidValue(message))
            }
        }
    }
}

/// Layered package filtering support.
#[derive(Debug, PartialEq, Eq, Clone)]
struct PkgFilters(IndexSet<PkgFilter>);

impl PkgFilters {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        repo: &EbuildRepo,
        val: R,
    ) -> Box<dyn Iterator<Item = EbuildPkg> + '_> {
        let mut iter: Box<dyn Iterator<Item = EbuildPkg>> =
            Box::new(repo.iter_restrict(val).filter_map(Result::ok));

        for f in &self.0 {
            iter = f.filter(iter);
        }

        iter
    }

    fn iter_restrict_ordered<R: Into<Restrict>>(
        &self,
        repo: &EbuildRepo,
        val: R,
    ) -> Box<dyn Iterator<Item = EbuildPkg> + '_> {
        let mut iter: Box<dyn Iterator<Item = EbuildPkg>> =
            Box::new(repo.iter_restrict_ordered(val).filter_map(Result::ok));

        for f in &self.0 {
            iter = f.filter(iter);
        }

        iter
    }
}

pub(crate) trait Source: fmt::Display {
    type Item;

    /// Return the [`SourceKind`] for the source.
    fn kind(&self) -> SourceKind;

    /// Return true of the source is filtered, otherwise false.
    fn is_filtered(&self) -> bool;

    /// Return the iterator of items matching a restriction.
    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + '_;

    /// Return the parallelized, ordered iterator of items matching a restriction.
    fn iter_restrict_ordered<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + '_;
}

pub(crate) struct EbuildPkgSource {
    repo: EbuildRepo,
    filters: PkgFilters,
}

impl EbuildPkgSource {
    pub(crate) fn new(repo: EbuildRepo, filters: IndexSet<PkgFilter>) -> Self {
        Self {
            repo,
            filters: PkgFilters(filters),
        }
    }
}

impl fmt::Display for EbuildPkgSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl Source for EbuildPkgSource {
    type Item = pkgcraft::Result<EbuildPkg>;

    fn kind(&self) -> SourceKind {
        SourceKind::EbuildPkg
    }

    fn is_filtered(&self) -> bool {
        !self.filters.is_empty()
    }

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + '_ {
        if self.is_filtered() {
            Either::Left(
                self.filters
                    .iter_restrict(&self.repo, val)
                    .flat_map(|pkg| self.repo.iter_restrict(&pkg)),
            )
        } else {
            Either::Right(self.repo.iter_restrict(val))
        }
    }

    fn iter_restrict_ordered<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + '_ {
        if self.is_filtered() {
            Either::Left(
                self.filters
                    .iter_restrict_ordered(&self.repo, val)
                    .flat_map(|pkg| self.repo.iter_restrict_ordered(&pkg)),
            )
        } else {
            Either::Right(self.repo.iter_restrict_ordered(val))
        }
    }
}

pub(crate) struct EbuildRawPkgSource {
    repo: EbuildRepo,
    filters: PkgFilters,
}

impl EbuildRawPkgSource {
    pub(crate) fn new(repo: EbuildRepo, filters: IndexSet<PkgFilter>) -> Self {
        Self {
            repo,
            filters: PkgFilters(filters),
        }
    }
}

impl fmt::Display for EbuildRawPkgSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl Source for EbuildRawPkgSource {
    type Item = pkgcraft::Result<EbuildRawPkg>;

    fn kind(&self) -> SourceKind {
        SourceKind::EbuildRawPkg
    }

    fn is_filtered(&self) -> bool {
        !self.filters.is_empty()
    }

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + '_ {
        if self.is_filtered() {
            Either::Left(
                self.filters
                    .iter_restrict(&self.repo, val)
                    .flat_map(|pkg| self.repo.iter_raw_restrict(&pkg)),
            )
        } else {
            Either::Right(self.repo.iter_raw_restrict(val))
        }
    }

    fn iter_restrict_ordered<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + '_ {
        if self.is_filtered() {
            Either::Left(
                self.filters
                    .iter_restrict_ordered(&self.repo, val)
                    .flat_map(|pkg| self.repo.iter_raw_restrict_ordered(&pkg)),
            )
        } else {
            Either::Right(self.repo.iter_raw_restrict_ordered(val))
        }
    }
}

/// Cache used to avoid recreating package objects for package and version scope scans.
#[derive(Debug)]
pub(crate) struct PkgCache<T> {
    cache: IndexMap<Cpv, pkgcraft::Result<T>>,
    pkgs: pkgcraft::Result<Vec<T>>,
}

impl<T: Package + Clone> PkgCache<T> {
    /// Create a new package cache from a source and restriction.
    pub(crate) fn new<S>(source: &S, scope: Scope, restrict: &Restrict) -> Self
    where
        S: Source<Item = pkgcraft::Result<T>>,
    {
        let mut cache = IndexMap::new();

        // create pkg cache when running in pkg or version scope
        if scope <= Scope::Package {
            for result in source.iter_restrict_ordered(restrict) {
                match &result {
                    Ok(pkg) => {
                        cache.insert(pkg.cpv().clone(), result);
                    }
                    Err(InvalidPkg { cpv, .. }) => {
                        cache.insert(*cpv.clone(), result);
                    }
                    Err(e) => unreachable!("unhandled metadata error: {e}"),
                }
            }
        }

        // only collect set in unfiltered package scope, in all other scopes it's not used
        let pkgs = if scope == Scope::Package && !source.is_filtered() {
            cache.values().cloned().try_collect()
        } else {
            Ok(Default::default())
        };

        Self { cache, pkgs }
    }

    /// Get all packages from the cache if none were invalid on creation.
    pub(crate) fn get_pkgs(&self) -> Result<&[T], &pkgcraft::Error> {
        self.pkgs.as_deref()
    }

    /// Get a matching package result from the cache if it exists.
    pub(crate) fn get_pkg(&self, cpv: &Cpv) -> Option<&pkgcraft::Result<T>> {
        self.cache.get(cpv)
    }
}
