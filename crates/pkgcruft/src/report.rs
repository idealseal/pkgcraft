use std::cmp::Ordering;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;

use camino::Utf8Path;
use colored::Color;
use indexmap::IndexSet;
use pkgcraft::bash::Node;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::repo::{ebuild::EbuildRepo, Repository};
use pkgcraft::restrict::{Restrict, Restriction, Scope};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

use crate::check::Check;
use crate::scanner::ReportFilter;
use crate::Error;

/// The severity of the report.
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

/// Report aliases that may related to one or more report variants.
///
/// Supports @Check, %ReportLevel, and Report variants.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum ReportAlias {
    Check(Check),
    Level(ReportLevel),
    Report(ReportKind),
    Scope(Scope),
}

impl From<Check> for ReportAlias {
    fn from(value: Check) -> Self {
        Self::Check(value)
    }
}

impl From<ReportLevel> for ReportAlias {
    fn from(value: ReportLevel) -> Self {
        Self::Level(value)
    }
}

impl From<ReportKind> for ReportAlias {
    fn from(value: ReportKind) -> Self {
        Self::Report(value)
    }
}

impl From<Scope> for ReportAlias {
    fn from(value: Scope) -> Self {
        Self::Scope(value)
    }
}

impl FromStr for ReportAlias {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(val) = s.strip_prefix('@') {
            val.parse().map(Self::Check)
        } else if let Some(val) = s.strip_prefix('%') {
            val.parse()
                .map(Self::Level)
                .map_err(|_| Error::InvalidValue(format!("invalid level: {val}")))
        } else if let Some(val) = s.strip_prefix('.') {
            val.parse()
                .map(Self::Scope)
                .map_err(|_| Error::InvalidValue(format!("invalid scope: {val}")))
        } else {
            s.parse()
                .map(Self::Report)
                .map_err(|_| Error::InvalidValue(format!("invalid report alias: {s}")))
        }
    }
}

impl ReportAlias {
    /// Expand a report alias into an iterator of its variants.
    pub fn expand(
        self,
        defaults: &IndexSet<ReportKind>,
    ) -> Box<dyn Iterator<Item = ReportKind> + '_> {
        match self {
            Self::Check(check) => Box::new(check.reports.iter().copied()),
            Self::Level(level) => {
                Box::new(defaults.iter().filter(move |r| r.level() == level).copied())
            }
            Self::Report(kind) => Box::new([kind].into_iter()),
            Self::Scope(scope) => {
                Box::new(defaults.iter().filter(move |r| r.scope() == scope).copied())
            }
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
    /// Arches that are unused.
    ArchesUnused,

    /// Ebuild uses a bash builtin as an external command.
    Builtin,

    /// Package dependency flagged as deprecated by the repo.
    DependencyDeprecated,

    /// Ebuild has an invalid dependency.
    DependencyInvalid,

    /// Package dependency is missing a revision.
    DependencyRevisionMissing,

    /// Dependency is missing a slot.
    DependencySlotMissing,

    /// Package has a banned EAPI.
    EapiBanned,

    /// Package has a deprecated EAPI.
    EapiDeprecated,

    /// Ebuild has a non-standard EAPI assignment format.
    ///
    /// The EAPI assignment should be wrapped in empty lines (except when the first line
    /// of the ebuild) with no whitespace prefix.
    EapiFormat,

    /// Package has an older EAPI than the previous release in the same SLOT.
    EapiStale,

    /// Package has stable keywords with an unstable EAPI.
    EapiUnstable,

    /// Ebuild file has a mismatched package name or invalid version.
    EbuildNameInvalid,

    /// Multiple ebuild versions for a package are equivalent, e.g. 0 and 0-r0.
    EbuildVersionsEqual,

    /// Eclass that is unused in the parent repository.
    EclassUnused,

    /// Usage of a nonexistent file in $FILESDIR.
    FileUnknown,

    /// Package has unused files in $FILESDIR.
    FilesUnused,

    /// File has an invalid copyright and/or license header.
    HeaderInvalid,

    /// Ebuild has an invalid homepage.
    HomepageInvalid,

    /// Ebuild has an invalid USE flag.
    IuseInvalid,

    /// Keywords have been dropped between releases.
    KeywordsDropped,

    /// Live ebuild has keywords.
    KeywordsLive,

    /// Ebuild has overlapping keywords.
    KeywordsOverlapping,

    /// Ebuild has unsorted keywords.
    KeywordsUnsorted,

    /// Ebuild has a deprecated license.
    LicenseDeprecated,

    /// Ebuild has an invalid license.
    LicenseInvalid,

    /// Repo has unused licenses.
    LicensesUnused,

    /// Package only has live ebuilds.
    LiveOnly,

    /// Package manifest has a matching hash with a different file name.
    ManifestCollide,

    /// Package manifest has matching file name with different hash.
    ManifestConflict,

    /// Package manifest is invalid.
    ManifestInvalid,

    /// Ebuild fails during metadata generation.
    MetadataError,

    /// Repo has unused mirrors.
    MirrorsUnused,

    /// Ebuild has an issue with optfeature usage.
    ///
    /// See the optfeature eclass for usage examples.
    Optfeature,

    /// Repo has unused profiles/package.deprecated entries.
    PackageDeprecatedUnused,

    /// Overlay package matches the name of a package from a parent repo.
    PackageOverride,

    /// Ebuild has invalid PROPERTIES.
    PropertiesInvalid,

    /// Ebuild can support newer python version(s).
    PythonUpdate,

    /// Empty category directory in a repository.
    RepoCategoryEmpty,

    /// Empty package directory in a repository.
    RepoPackageEmpty,

    /// Ebuild has invalid RESTRICT.
    RestrictInvalid,

    /// Ebuild is missing a RESTRICT value of the specified type.
    RestrictMissing,

    /// Ebuild can support newer ruby version(s).
    RubyUpdate,

    /// Package only has unstable keywords.
    UnstableOnly,

    /// Ebuild has an unsupported or invalid URI.
    UriInvalid,

    /// Global USE flags that are unused.
    UseGlobalUnused,

    /// Local USE flag missing description.
    UseLocalDescMissing,

    /// Local USE flag description matching a global USE flag.
    UseLocalGlobal,

    /// Local USE flag that is unsorted.
    UseLocalUnsorted,

    /// Local USE flag that is unused.
    UseLocalUnused,

    /// Global metadata variables are defined in non-standard order.
    ///
    /// Note that this only is reported for ebuilds with all target variables
    /// unconditionally defined in global scope.
    VariableOrder,

    /// Whitespace usage that is invalid.
    WhitespaceInvalid,

    /// Whitespace usage that isn't needed.
    WhitespaceUnneeded,
}

impl ReportKind {
    /// Create a version scope report.
    pub(crate) fn version<T: Into<Cpv>>(self, value: T) -> ReportBuilder {
        ReportBuilder(Report {
            kind: self,
            scope: ReportScope::Version(value.into(), None),
            message: Default::default(),
        })
    }

    /// Create a package scope report.
    pub(crate) fn package<T>(self, value: T) -> ReportBuilder
    where
        T: TryInto<Cpn>,
        <T as TryInto<Cpn>>::Error: fmt::Display,
    {
        let cpn = value
            .try_into()
            .unwrap_or_else(|e| unreachable!("can't convert value to Cpn: {e}"));

        ReportBuilder(Report {
            kind: self,
            scope: ReportScope::Package(cpn),
            message: Default::default(),
        })
    }

    /// Create a category scope report.
    pub(crate) fn category<S: fmt::Display>(self, value: S) -> ReportBuilder {
        ReportBuilder(Report {
            kind: self,
            scope: ReportScope::Category(value.to_string()),
            message: Default::default(),
        })
    }

    /// Create a repo scope report.
    pub(crate) fn repo<R: Repository>(self, repo: R) -> ReportBuilder {
        ReportBuilder(Report {
            kind: self,
            scope: ReportScope::Repo(repo.name().to_string()),
            message: Default::default(),
        })
    }

    /// Return the severity level of the report variant.
    pub fn level(&self) -> ReportLevel {
        use ReportLevel::*;
        match self {
            Self::ArchesUnused => Warning,
            Self::Builtin => Error,
            Self::DependencyDeprecated => Warning,
            Self::DependencyInvalid => Error,
            Self::DependencyRevisionMissing => Warning,
            Self::DependencySlotMissing => Warning,
            Self::EapiBanned => Error,
            Self::EapiDeprecated => Warning,
            Self::EapiFormat => Style,
            Self::EapiStale => Warning,
            Self::EapiUnstable => Error,
            Self::EbuildNameInvalid => Error,
            Self::EbuildVersionsEqual => Error,
            Self::EclassUnused => Warning,
            Self::FileUnknown => Error,
            Self::FilesUnused => Warning,
            Self::HeaderInvalid => Error,
            Self::HomepageInvalid => Error,
            Self::IuseInvalid => Error,
            Self::KeywordsDropped => Warning,
            Self::KeywordsLive => Warning,
            Self::KeywordsOverlapping => Error,
            Self::KeywordsUnsorted => Style,
            Self::LicenseDeprecated => Warning,
            Self::LicenseInvalid => Error,
            Self::LicensesUnused => Warning,
            Self::LiveOnly => Warning,
            Self::ManifestInvalid => Error,
            Self::ManifestCollide => Warning,
            Self::ManifestConflict => Error,
            Self::MetadataError => Critical,
            Self::MirrorsUnused => Warning,
            Self::Optfeature => Warning,
            Self::PackageDeprecatedUnused => Warning,
            Self::PackageOverride => Warning,
            Self::PropertiesInvalid => Error,
            Self::PythonUpdate => Info,
            Self::RepoCategoryEmpty => Warning,
            Self::RepoPackageEmpty => Warning,
            Self::RestrictInvalid => Error,
            Self::RestrictMissing => Warning,
            Self::RubyUpdate => Info,
            Self::UnstableOnly => Info,
            Self::UriInvalid => Error,
            Self::UseGlobalUnused => Warning,
            Self::UseLocalDescMissing => Error,
            Self::UseLocalGlobal => Warning,
            Self::UseLocalUnsorted => Style,
            Self::UseLocalUnused => Warning,
            Self::VariableOrder => Style,
            Self::WhitespaceInvalid => Warning,
            Self::WhitespaceUnneeded => Style,
        }
    }

    /// Return the scope of the report variant.
    pub fn scope(&self) -> Scope {
        use Scope::*;
        match self {
            Self::ArchesUnused => Repo,
            Self::Builtin => Version,
            Self::DependencyDeprecated => Version,
            Self::DependencyInvalid => Version,
            Self::DependencyRevisionMissing => Version,
            Self::DependencySlotMissing => Version,
            Self::EapiBanned => Version,
            Self::EapiDeprecated => Version,
            Self::EapiFormat => Version,
            Self::EapiStale => Version,
            Self::EapiUnstable => Version,
            Self::EbuildNameInvalid => Package,
            Self::EbuildVersionsEqual => Package,
            Self::EclassUnused => Version,
            Self::FileUnknown => Version,
            Self::FilesUnused => Package,
            Self::HeaderInvalid => Version,
            Self::HomepageInvalid => Version,
            Self::IuseInvalid => Version,
            Self::KeywordsDropped => Version,
            Self::KeywordsLive => Version,
            Self::KeywordsOverlapping => Version,
            Self::KeywordsUnsorted => Version,
            Self::LicenseDeprecated => Version,
            Self::LicenseInvalid => Version,
            Self::LicensesUnused => Repo,
            Self::LiveOnly => Package,
            Self::ManifestInvalid => Package,
            Self::ManifestCollide => Repo,
            Self::ManifestConflict => Repo,
            Self::MetadataError => Version,
            Self::MirrorsUnused => Repo,
            Self::Optfeature => Version,
            Self::PackageDeprecatedUnused => Repo,
            Self::PackageOverride => Package,
            Self::PropertiesInvalid => Version,
            Self::PythonUpdate => Version,
            Self::RepoCategoryEmpty => Category,
            Self::RepoPackageEmpty => Package,
            Self::RestrictInvalid => Version,
            Self::RestrictMissing => Version,
            Self::RubyUpdate => Version,
            Self::UnstableOnly => Package,
            Self::UriInvalid => Version,
            Self::UseGlobalUnused => Repo,
            Self::UseLocalDescMissing => Package,
            Self::UseLocalGlobal => Package,
            Self::UseLocalUnsorted => Package,
            Self::UseLocalUnused => Package,
            Self::VariableOrder => Version,
            Self::WhitespaceInvalid => Version,
            Self::WhitespaceUnneeded => Version,
        }
    }

    /// Return an iterator of all report variants enabled by default.
    pub fn iter_default(repo: &EbuildRepo) -> impl Iterator<Item = Self> + '_ {
        Check::iter_default(repo).flat_map(|x| x.reports).copied()
    }
}

/// Builder for reports.
pub(crate) struct ReportBuilder(Report);

impl ReportBuilder {
    /// Add a report message.
    pub(crate) fn message<S>(mut self, value: S) -> Self
    where
        S: fmt::Display,
    {
        self.0.message = Some(value.to_string());
        self
    }

    /// Add a location reference.
    pub(crate) fn location<L>(mut self, value: L) -> Self
    where
        L: Into<Location>,
    {
        let ReportScope::Version(_, location @ None) = &mut self.0.scope else {
            unreachable!("invalid report scope: {:?}", self.0.scope);
        };

        *location = Some(value.into());
        self
    }

    /// Pass the report to the scanning filter for processing.
    pub(crate) fn report(self, filter: &mut ReportFilter) {
        // verify report variant scope matches generated scope
        debug_assert!(
            self.0.kind.scope() == self.0.scope.scope(),
            "{} scope: {} != {}",
            self.0.kind,
            self.0.kind.scope(),
            self.0.scope.scope(),
        );

        // submit report
        filter.report(self.0)
    }
}

/// A position in a multi-line text file, in terms of lines and columns.
///
/// Values are not zero-based so a value of zero means the field is unset.
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line {}", self.line)?;
        if self.column > 0 {
            write!(f, ", column {}", self.column)?;
        }
        Ok(())
    }
}

impl From<usize> for Location {
    fn from(value: usize) -> Self {
        Self { line: value, column: 0 }
    }
}

impl From<(usize, usize)> for Location {
    fn from(value: (usize, usize)) -> Self {
        Self { line: value.0, column: value.1 }
    }
}

impl From<&Node<'_>> for Location {
    fn from(value: &Node<'_>) -> Self {
        Self {
            line: value.start_position().row + 1,
            column: value.start_position().column + 1,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum ReportScope {
    Version(Cpv, Option<Location>),
    Package(Cpn),
    Category(String),
    Repo(String),
}

impl ReportScope {
    fn scope(&self) -> Scope {
        match self {
            Self::Version(_, _) => Scope::Version,
            Self::Package(_) => Scope::Package,
            Self::Category(_) => Scope::Category,
            Self::Repo(_) => Scope::Repo,
        }
    }
}

impl Ord for ReportScope {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Repo(v1), Self::Repo(v2)) => v1.cmp(v2),
            (Self::Category(v1), Self::Category(v2)) => v1.cmp(v2),
            (Self::Package(v1), Self::Package(v2)) => v1.cmp(v2),
            (Self::Version(v1, l1), Self::Version(v2, l2)) => {
                v1.cmp(v2).then_with(|| l1.cmp(l2))
            }
            (Self::Version(v1, _), Self::Package(v2)) => v1
                .cpn()
                .cmp(v2)
                .then_with(|| self.scope().cmp(&other.scope())),
            (Self::Package(v1), Self::Version(v2, _)) => v1
                .cmp(v2.cpn())
                .then_with(|| self.scope().cmp(&other.scope())),
            _ => self.scope().cmp(&other.scope()),
        }
    }
}

impl PartialOrd for ReportScope {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Debug for ReportScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Version(cpv, Some(location)) => {
                write!(f, "Version( {cpv}, {location:?} )")
            }
            Self::Version(cpv, None) => write!(f, "Version( {cpv} )"),
            Self::Package(cpn) => write!(f, "Package( {cpn} )"),
            Self::Category(cat) => write!(f, "Category( {cat} )"),
            Self::Repo(repo) => write!(f, "Repo( {repo} )"),
        }
    }
}

impl fmt::Display for ReportScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Version(cpv, Some(location)) => write!(f, "{cpv}, {location}"),
            Self::Version(cpv, None) => write!(f, "{cpv}"),
            Self::Package(cpn) => write!(f, "{cpn}"),
            Self::Category(cat) => write!(f, "{cat}"),
            Self::Repo(repo) => write!(f, "{repo}"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub struct Report {
    kind: ReportKind,
    scope: ReportScope,
    message: Option<String>,
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

    /// The report message.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
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
        self.scope
            .cmp(&other.scope)
            .then_with(|| self.kind.cmp(&other.kind))
            .then_with(|| self.message.cmp(&other.message))
    }
}

impl PartialOrd for Report {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.scope, self.kind)?;
        if let Some(value) = self.message() {
            write!(f, ": {value}")?;
        }
        Ok(())
    }
}

impl Restriction<&Report> for Restrict {
    fn matches(&self, report: &Report) -> bool {
        match &report.scope {
            ReportScope::Version(cpv, _) => self.matches(cpv),
            ReportScope::Package(cpn) => self.matches(cpn),
            _ => false,
        }
    }
}

/// Iterator for deserializing reports from a BufRead object.
pub struct Iter<'a, R: BufRead> {
    reader: R,
    line: String,
    reports: Option<&'a IndexSet<ReportKind>>,
    restrict: Option<&'a Restrict>,
    scopes: Option<&'a IndexSet<Scope>>,
}

impl<'a> Iter<'a, BufReader<File>> {
    /// Try to create a new reports iterator from a file path.
    pub fn try_from_file<P: AsRef<Utf8Path>>(
        path: P,
        reports: Option<&'a IndexSet<ReportKind>>,
        restrict: Option<&'a Restrict>,
        scopes: Option<&'a IndexSet<Scope>>,
    ) -> crate::Result<Iter<'a, BufReader<File>>> {
        let path = path.as_ref();
        let file = File::open(path)
            .map_err(|e| Error::InvalidValue(format!("failed loading file: {path}: {e}")))?;
        Ok(Iter {
            reader: BufReader::new(file),
            line: String::new(),
            reports,
            restrict,
            scopes,
        })
    }
}

impl<'a, R: BufRead> Iter<'a, R> {
    /// Create a new reports iterator from a BufRead object.
    pub fn from_reader(
        reader: R,
        reports: Option<&'a IndexSet<ReportKind>>,
        restrict: Option<&'a Restrict>,
        scopes: Option<&'a IndexSet<Scope>>,
    ) -> Iter<'a, R> {
        Iter {
            reader,
            line: String::new(),
            reports,
            restrict,
            scopes,
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

        // skip excluded scope variants
        if let Some(scopes) = self.scopes {
            if !scopes.contains(&report.scope().scope()) {
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

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use pkgcraft::test::assert_ordered_eq;
    use pretty_assertions::assert_eq;
    use strum::IntoEnumIterator;

    use super::*;

    // serialized reports in order
    static REPORTS: &str = indoc::indoc! {r#"
        {"kind":"DependencyDeprecated","scope":{"Version":["cat/pkg1-2-r3",null]},"message":"BDEPEND: cat/deprecated"}
        {"kind":"EapiDeprecated","scope":{"Version":["cat/pkg1-2-r3",null]},"message":"6"}
        {"kind":"WhitespaceUnneeded","scope":{"Version":["cat/pkg1-2-r3",{"line":3,"column":0}]},"message":"empty line"}
        {"kind":"WhitespaceInvalid","scope":{"Version":["cat/pkg1-2-r3",{"line":6,"column":0}]},"message":"missing ending newline"}
        {"kind":"UnstableOnly","scope":{"Package":"cat/pkg1"},"message":"arch1"}
        {"kind":"UnstableOnly","scope":{"Package":"cat/pkg1"},"message":"arch2"}
        {"kind":"EapiDeprecated","scope":{"Version":["cat/pkg2-1-r2",null]},"message":"6"}
        {"kind":"RepoCategoryEmpty","scope":{"Category":"cat1"},"message":null}
        {"kind":"RepoCategoryEmpty","scope":{"Category":"cat2"},"message":null}
        {"kind":"LicensesUnused","scope":{"Repo":"repo1"},"message":"unused"}
        {"kind":"LicensesUnused","scope":{"Repo":"repo2"},"message":"unused"}
    "#};

    #[test]
    fn kind() {
        // verify ReportKind are kept in lexical order
        let kinds: Vec<_> = ReportKind::iter().collect();
        let ordered: Vec<_> = ReportKind::iter().map(|x| x.to_string()).sorted().collect();
        let ordered: Vec<_> = ordered.iter().map(|s| s.parse().unwrap()).collect();
        assert_eq!(&kinds, &ordered, "unordered ReportKind variants");
    }

    #[test]
    fn cmp() {
        // deserialize reports
        let expected: Vec<_> = REPORTS
            .lines()
            .map(Report::from_json)
            .try_collect()
            .unwrap();

        // verify ordering manually for PartialOrd tests
        for (a, b) in expected.iter().tuples() {
            assert!(a < b);
            assert!(a.scope() <= b.scope());
        }

        // reverse reports and sort them back into the expected order
        let mut reports = expected.clone();
        reports.reverse();
        reports.sort();

        // compare reports via string serialization for better diff output
        let expected = expected.iter().map(|r| r.to_string());
        let reports = reports.iter().map(|r| r.to_string());
        assert_ordered_eq!(expected, reports);
    }

    #[test]
    fn display_and_debug() {
        for report in REPORTS.lines().filter_map(|s| Report::from_json(s).ok()) {
            let kind = report.kind().to_string();
            let scope = report.scope().to_string();

            // regular output
            let s = report.to_string();
            assert!(s.contains(&kind));
            assert!(s.contains(&scope));

            // debug output
            let s = format!("{report:?}");
            assert!(s.contains(&kind));
            assert!(s.contains(&scope));
        }
    }
}
