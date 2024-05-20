use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};

use camino::Utf8Path;
use colored::Color;
use indexmap::IndexSet;
use once_cell::sync::Lazy;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::macros::cmp_not_equal;
use pkgcraft::pkg::Package;
use pkgcraft::restrict::{Restrict, Restriction};
use pkgcraft::types::{OrderedMap, OrderedSet};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator, VariantNames};

use crate::check::CheckKind;
use crate::scope::Scope;
use crate::Error;

/// The severity of the report.
#[derive(
    AsRefStr,
    Display,
    EnumIter,
    EnumString,
    VariantNames,
    Serialize,
    Deserialize,
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
pub enum ReportLevel {
    Critical,
    Error,
    Warning,
    Style,
    Info,
}

impl From<ReportLevel> for Color {
    fn from(level: ReportLevel) -> Self {
        match level {
            ReportLevel::Critical => Color::Red,
            ReportLevel::Error => Color::TrueColor { r: 255, g: 140, b: 0 },
            ReportLevel::Warning => Color::Yellow,
            ReportLevel::Style => Color::Cyan,
            ReportLevel::Info => Color::Green,
        }
    }
}

/// Report variants.
#[derive(
    Serialize,
    Deserialize,
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
pub enum ReportKind {
    DeprecatedDependency,
    DroppedKeywords,

    /// Package has a banned EAPI.
    EapiBanned,

    /// Package has a deprecated EAPI.
    EapiDeprecated,

    /// Package has stable keywords with an unstable EAPI.
    EapiUnstable,

    InvalidDependencySet,
    MissingMetadata,
    MissingRevision,
    OverlappingKeywords,
    SourcingError,
    UnsortedKeywords,
    UnstableOnly,
}

impl ReportKind {
    /// Create a version scope report.
    pub(crate) fn version<P, S>(self, pkg: P, description: S) -> Report
    where
        P: Package,
        S: fmt::Display,
    {
        Report {
            kind: self,
            scope: ReportScope::Version(pkg.cpv().clone()),
            description: description.to_string(),
        }
    }

    /// Create a package scope report.
    pub(crate) fn package<P, S>(self, pkgs: &[P], description: S) -> Report
    where
        P: Package,
        S: fmt::Display,
    {
        Report {
            kind: self,
            scope: ReportScope::Package(pkgs[0].cpn().clone()),
            description: description.to_string(),
        }
    }

    /// Return the scope of the report variant.
    pub fn scope(&self) -> Scope {
        match self {
            Self::DeprecatedDependency => Scope::Version,
            Self::DroppedKeywords => Scope::Version,
            Self::EapiBanned => Scope::Version,
            Self::EapiDeprecated => Scope::Version,
            Self::EapiUnstable => Scope::Version,
            Self::InvalidDependencySet => Scope::Version,
            Self::MissingMetadata => Scope::Version,
            Self::MissingRevision => Scope::Version,
            Self::OverlappingKeywords => Scope::Version,
            Self::SourcingError => Scope::Version,
            Self::UnsortedKeywords => Scope::Version,
            Self::UnstableOnly => Scope::Package,
        }
    }

    /// Return the severity level of the report variant.
    pub fn level(&self) -> ReportLevel {
        use ReportLevel::*;
        match self {
            Self::DeprecatedDependency => Warning,
            Self::DroppedKeywords => Warning,
            Self::EapiBanned => Error,
            Self::EapiDeprecated => Warning,
            Self::EapiUnstable => Error,
            Self::InvalidDependencySet => Critical,
            Self::MissingMetadata => Critical,
            Self::MissingRevision => Warning,
            Self::OverlappingKeywords => Warning,
            Self::SourcingError => Critical,
            Self::UnsortedKeywords => Style,
            Self::UnstableOnly => Info,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub enum ReportScope {
    Version(Cpv<String>),
    Package(Cpn<String>),
}

impl fmt::Display for ReportScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Version(cpv) => write!(f, "{cpv}"),
            Self::Package(cpn) => write!(f, "{cpn}"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub struct Report {
    kind: ReportKind,
    scope: ReportScope,
    description: String,
}

impl Report {
    /// The report variant.
    pub fn kind(&self) -> &ReportKind {
        &self.kind
    }

    /// The scope the report relates to, e.g. a specific package version or package name.
    pub fn scope(&self) -> &ReportScope {
        &self.scope
    }

    /// The description of the report.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The severity of the report.
    pub fn level(&self) -> ReportLevel {
        self.kind.level()
    }

    /// Serialize a [`Report`] into a JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self).expect("failed serializing report")
    }

    /// Deserialize a JSON string into a [`Report`].
    pub fn from_json(data: &str) -> crate::Result<Self> {
        serde_json::from_str(data)
            .map_err(|e| Error::InvalidValue(format!("failed deserializing report: {e}")))
    }
}

impl Ord for Report {
    fn cmp(&self, other: &Self) -> Ordering {
        use ReportScope::*;
        match (&self.scope, &other.scope) {
            (Version(cpv), Package(dep)) => {
                cmp_not_equal!(&(cpv.category(), cpv.package()), &(dep.category(), dep.package()));
                return Ordering::Less;
            }
            (Package(dep), Version(cpv)) => {
                cmp_not_equal!(&(dep.category(), dep.package()), &(cpv.category(), cpv.package()));
                return Ordering::Greater;
            }
            (s1, s2) => cmp_not_equal!(s1, s2),
        }
        cmp_not_equal!(&self.kind, &other.kind);
        self.description.cmp(&other.description)
    }
}

impl PartialOrd for Report {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Restriction<&Report> for Restrict {
    fn matches(&self, report: &Report) -> bool {
        match &report.scope {
            ReportScope::Version(cpv) => self.matches(cpv),
            ReportScope::Package(cpn) => self.matches(cpn),
        }
    }
}

/// Iterator for deserializing reports from a BufRead object.
pub struct Iter<'a, R: BufRead> {
    reader: R,
    line: String,
    reports: Option<&'a HashSet<ReportKind>>,
    restrict: Option<&'a Restrict>,
}

impl<'a> Iter<'a, BufReader<File>> {
    /// Try to create a new reports iterator from a file path.
    pub fn try_from_file<P: AsRef<Utf8Path>>(
        path: P,
        reports: Option<&'a HashSet<ReportKind>>,
        restrict: Option<&'a Restrict>,
    ) -> crate::Result<Iter<'a, BufReader<File>>> {
        let path = path.as_ref();
        let file = File::open(path)
            .map_err(|e| Error::InvalidValue(format!("failed loading file: {path}: {e}")))?;
        Ok(Iter {
            reader: BufReader::new(file),
            line: String::new(),
            reports,
            restrict,
        })
    }
}

impl<'a, R: BufRead> Iter<'a, R> {
    /// Create a new reports iterator from a BufRead object.
    pub fn from_reader(
        reader: R,
        reports: Option<&'a HashSet<ReportKind>>,
        restrict: Option<&'a Restrict>,
    ) -> Iter<'a, R> {
        Iter {
            reader,
            line: String::new(),
            reports,
            restrict,
        }
    }

    /// Determine if a given [`Report`] should be filtered.
    fn filtered(&self, report: &Report) -> bool {
        // skip excluded report variants
        if let Some(reports) = self.reports {
            if !reports.contains(report.kind()) {
                return true;
            }
        }

        // skip excluded restrictions
        if let Some(filter) = self.restrict {
            if !filter.matches(report) {
                return true;
            }
        }

        false
    }
}

impl<R: BufRead> Iterator for Iter<'_, R> {
    type Item = crate::Result<Report>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.line.clear();
            match self.reader.read_line(&mut self.line) {
                Ok(0) => return None,
                Ok(_) => match Report::from_json(&self.line) {
                    Ok(report) => {
                        if !self.filtered(&report) {
                            return Some(Ok(report));
                        }
                    }
                    err => return Some(err),
                },
                Err(e) => {
                    return Some(Err(Error::InvalidValue(format!("failed reading line: {e}"))))
                }
            }
        }
    }
}

/// The ordered set of all report variants.
pub static REPORTS: Lazy<IndexSet<ReportKind>> = Lazy::new(|| {
    let mut reports: IndexSet<_> = CheckKind::iter()
        .flat_map(|c| c.check().reports.iter())
        .copied()
        .collect();
    reports.sort();
    reports
});

/// The ordered map of all report variants to the checks that can generate them.
pub static REPORT_CHECKS: Lazy<OrderedMap<ReportKind, OrderedSet<CheckKind>>> = Lazy::new(|| {
    let mut map: OrderedMap<_, OrderedSet<_>> = CheckKind::iter()
        .flat_map(|c| c.check().reports.iter().copied().map(move |r| (r, c)))
        .collect();
    map.sort_keys();
    map
});
