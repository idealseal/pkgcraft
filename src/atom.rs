use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt::{self, Write};
use std::str::FromStr;

use cached::{proc_macro::cached, SizedCache};
use indexmap::IndexSet;

pub use self::version::Version;
use self::version::{Operator, ParsedVersion};
use crate::eapi::{IntoEapi, EAPI_PKGCRAFT};
use crate::macros::{cmp_not_equal, vec_str};
use crate::restrict::{self, Restriction};
use crate::Error;
// export parser functionality
pub use parser::parse;

mod parser;
pub(crate) mod version;

type BaseRestrict = restrict::Restrict;

#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum Blocker {
    NONE,   // cat/pkg
    Strong, // !!cat/pkg
    Weak,   // !cat/pkg
}

// use the latest EAPI for the Default trait
impl Default for Blocker {
    fn default() -> Blocker {
        Blocker::NONE
    }
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Blocker::NONE => Ok(()),
            Blocker::Weak => write!(f, "!"),
            Blocker::Strong => write!(f, "!!"),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ParsedAtom<'a> {
    pub(crate) category: &'a str,
    pub(crate) package: &'a str,
    pub(crate) blocker: Blocker,
    pub(crate) version: Option<ParsedVersion<'a>>,
    pub(crate) version_str: Option<&'a str>,
    pub(crate) slot: Option<&'a str>,
    pub(crate) subslot: Option<&'a str>,
    pub(crate) slot_op: Option<&'a str>,
    pub(crate) use_deps: Option<Vec<&'a str>>,
    pub(crate) repo: Option<&'a str>,
}

impl ParsedAtom<'_> {
    pub(crate) fn into_owned(self) -> crate::Result<Atom> {
        let version = match (self.version, self.version_str) {
            (Some(v), Some(s)) => Some(v.into_owned(s)?),
            _ => None,
        };

        Ok(Atom {
            category: self.category.to_string(),
            package: self.package.to_string(),
            blocker: self.blocker,
            version,
            slot: self.slot.map(|s| s.to_string()),
            subslot: self.subslot.map(|s| s.to_string()),
            slot_op: self.slot_op.map(|s| s.to_string()),
            use_deps: self.use_deps.as_ref().map(|u| vec_str!(u)),
            repo: self.repo.map(|s| s.to_string()),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Atom {
    category: String,
    package: String,
    blocker: Blocker,
    version: Option<Version>,
    slot: Option<String>,
    subslot: Option<String>,
    slot_op: Option<String>,
    use_deps: Option<Vec<String>>,
    repo: Option<String>,
}

#[cached(
    type = "SizedCache<String, crate::Result<Atom>>",
    create = "{ SizedCache::with_size(1000) }",
    convert = r#"{ s.to_string() }"#
)]
/// Create a new Atom from a given CPV string (e.g. cat/pkg-1).
pub fn cpv(s: &str) -> crate::Result<Atom> {
    let mut atom = parse::cpv(s)?;
    atom.version_str = Some(s);
    atom.into_owned()
}

impl Atom {
    /// Verify a string represents a valid atom.
    pub fn valid<E: IntoEapi>(s: &str, eapi: E) -> crate::Result<()> {
        parse::dep_str(s, eapi.into_eapi()?)?;
        Ok(())
    }

    /// Verify a string represents a valid atom.
    pub fn valid_cpv(s: &str) -> crate::Result<()> {
        parse::cpv(s)?;
        Ok(())
    }

    /// Create a new Atom from a given string.
    pub fn new<E: IntoEapi>(s: &str, eapi: E) -> crate::Result<Self> {
        parse::dep(s, eapi.into_eapi()?)
    }

    pub fn category(&self) -> &str {
        &self.category
    }

    pub fn package(&self) -> &str {
        &self.package
    }

    pub fn blocker(&self) -> Blocker {
        self.blocker
    }

    fn use_deps_set(&self) -> IndexSet<&str> {
        match self.use_deps() {
            None => IndexSet::<&str>::new(),
            Some(u) => u.iter().copied().collect(),
        }
    }

    pub fn use_deps(&self) -> Option<Vec<&str>> {
        self.use_deps
            .as_ref()
            .map(|u| u.iter().map(|s| s.as_str()).collect())
    }

    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    pub fn revision(&self) -> Option<&version::Revision> {
        self.version.as_ref().map(|v| v.revision())
    }

    pub fn key(&self) -> String {
        format!("{}/{}", self.category, self.package)
    }

    pub fn cpv(&self) -> String {
        match &self.version {
            Some(ver) => format!("{}/{}-{ver}", self.category, self.package),
            None => format!("{}/{}", self.category, self.package),
        }
    }

    pub fn slot(&self) -> Option<&str> {
        self.slot.as_deref()
    }

    pub fn subslot(&self) -> Option<&str> {
        self.subslot.as_deref()
    }

    pub fn slot_op(&self) -> Option<&str> {
        self.slot_op.as_deref()
    }

    pub fn repo(&self) -> Option<&str> {
        self.repo.as_deref()
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();

        // append blocker
        write!(s, "{}", self.blocker)?;

        // append version operator with cpv
        let cpv = self.cpv();
        match self.version.as_ref().and_then(|v| v.op()) {
            Some(Operator::Less) => write!(s, "<{cpv}")?,
            Some(Operator::LessOrEqual) => write!(s, "<={cpv}")?,
            Some(Operator::Equal) => write!(s, "={cpv}")?,
            Some(Operator::EqualGlob) => write!(s, "={cpv}*")?,
            Some(Operator::Approximate) => write!(s, "~{cpv}")?,
            Some(Operator::GreaterOrEqual) => write!(s, ">={cpv}")?,
            Some(Operator::Greater) => write!(s, ">{cpv}")?,
            None => s.push_str(&cpv),
        }

        // append slot data
        match (self.slot(), self.subslot(), self.slot_op()) {
            (Some(slot), Some(subslot), Some(op)) => write!(s, ":{slot}/{subslot}{op}")?,
            (Some(slot), Some(subslot), None) => write!(s, ":{slot}/{subslot}")?,
            (Some(slot), None, Some(op)) => write!(s, ":{slot}{op}")?,
            (Some(x), None, None) | (None, None, Some(x)) => write!(s, ":{x}")?,
            _ => (),
        }

        // append use deps
        if let Some(x) = &self.use_deps {
            write!(s, "[{}]", &x.join(","))?;
        }

        // append repo
        if let Some(repo) = &self.repo {
            write!(s, "::{repo}")?;
        }

        write!(f, "{s}")
    }
}

impl Ord for Atom {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_not_equal!(&self.category, &other.category);
        cmp_not_equal!(&self.package, &other.package);
        cmp_not_equal!(&self.version, &other.version);
        cmp_not_equal!(&self.blocker, &other.blocker);
        cmp_not_equal!(&self.slot, &other.slot);
        cmp_not_equal!(&self.subslot, &other.subslot);
        cmp_not_equal!(&self.use_deps, &other.use_deps);
        self.repo.cmp(&other.repo)
    }
}

impl PartialOrd for Atom {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Atom {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Atom::new(s, &*EAPI_PKGCRAFT)
    }
}

#[derive(Debug, Clone)]
pub enum Restrict {
    Category(restrict::Str),
    Package(restrict::Str),
    Version(Option<Version>),
    VersionStr(restrict::Str),
    Slot(Option<restrict::Str>),
    SubSlot(Option<restrict::Str>),
    StaticUseDep(restrict::Set),
    Repo(Option<restrict::Str>),
}

impl Restrict {
    pub fn category(s: &str) -> BaseRestrict {
        let r = Restrict::Category(restrict::Str::Match(s.to_string()));
        BaseRestrict::Atom(r)
    }

    pub fn package(s: &str) -> BaseRestrict {
        let r = Restrict::Package(restrict::Str::Match(s.to_string()));
        BaseRestrict::Atom(r)
    }

    pub fn version(o: Option<&str>) -> crate::Result<BaseRestrict> {
        let o = match o {
            None => None,
            Some(s) => Some(Version::from_str(s)?),
        };
        let r = Restrict::Version(o);
        Ok(BaseRestrict::Atom(r))
    }

    pub fn slot(o: Option<&str>) -> BaseRestrict {
        let o = o.map(|s| restrict::Str::Match(s.to_string()));
        BaseRestrict::Atom(Restrict::Slot(o))
    }

    pub fn subslot(o: Option<&str>) -> BaseRestrict {
        let o = o.map(|s| restrict::Str::Match(s.to_string()));
        BaseRestrict::Atom(Restrict::SubSlot(o))
    }

    pub fn use_deps<I, S>(iter: I) -> BaseRestrict
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let r = Restrict::StaticUseDep(restrict::Set::StrSubset(
            iter.into_iter().map(|s| s.into()).collect(),
        ));
        BaseRestrict::Atom(r)
    }

    pub fn repo(o: Option<&str>) -> BaseRestrict {
        let o = o.map(|s| restrict::Str::Match(s.to_string()));
        BaseRestrict::Atom(Restrict::Repo(o))
    }
}

impl Restriction<&Atom> for Restrict {
    fn matches(&self, atom: &Atom) -> bool {
        match self {
            Self::Category(r) => r.matches(atom.category()),
            Self::Package(r) => r.matches(atom.package()),
            Self::Version(v) => match (v, atom.version()) {
                (Some(v), Some(ver)) => v.op_cmp(ver),
                (None, None) => true,
                _ => false,
            },
            Self::VersionStr(r) => r.matches(atom.version().map_or_else(|| "", |v| v.as_str())),
            Self::Slot(r) => match (r, atom.slot()) {
                (Some(r), Some(slot)) => r.matches(slot),
                (None, None) => true,
                _ => false,
            },
            Self::SubSlot(r) => match (r, atom.subslot()) {
                (Some(r), Some(subslot)) => r.matches(subslot),
                (None, None) => true,
                _ => false,
            },
            Self::StaticUseDep(r) => r.matches(&atom.use_deps_set()),
            Self::Repo(r) => match (r, atom.repo()) {
                (Some(r), Some(repo)) => r.matches(repo),
                (None, None) => true,
                _ => false,
            },
        }
    }
}

impl Restriction<&Atom> for BaseRestrict {
    fn matches(&self, atom: &Atom) -> bool {
        crate::restrict::restrict_match! {
            self, atom,
            Self::Atom(r) => r.matches(atom)
        }
    }
}

impl<T: Borrow<Atom>> From<T> for BaseRestrict {
    fn from(atom: T) -> Self {
        let atom = atom.borrow();
        let mut restricts =
            vec![Restrict::category(atom.category()), Restrict::package(atom.package())];

        if let Some(v) = atom.version() {
            restricts.push(Self::Atom(Restrict::Version(Some(v.clone()))));
        }

        if let Some(s) = atom.slot() {
            restricts.push(Restrict::slot(Some(s)));
        }

        if let Some(s) = atom.subslot() {
            restricts.push(Restrict::subslot(Some(s)));
        }

        // TODO: add use deps support

        if let Some(s) = atom.repo() {
            restricts.push(Restrict::repo(Some(s)));
        }

        Self::and(restricts)
    }
}

#[cfg(test)]
mod tests {
    use crate::test::Atoms;

    use super::*;

    #[test]
    fn test_fmt() {
        let mut atom: Atom;
        for s in [
            "cat/pkg",
            "<cat/pkg-4",
            "<=cat/pkg-4-r1",
            "=cat/pkg-4-r0",
            "=cat/pkg-4-r01",
            "=cat/pkg-4*",
            "~cat/pkg-4",
            ">=cat/pkg-r1-2-r3",
            ">cat/pkg-4-r1:0=",
            ">cat/pkg-4-r1:0/2=[use]",
            ">cat/pkg-4-r1:0/2=[use]::repo",
            "!cat/pkg",
            "!!<cat/pkg-4",
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(format!("{atom}"), s);
        }
    }

    #[test]
    fn test_atom_key() {
        let mut atom: Atom;
        for (s, key) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg"),
            ("<=cat/pkg-4-r1", "cat/pkg"),
            ("=cat/pkg-4", "cat/pkg"),
            ("=cat/pkg-4*", "cat/pkg"),
            ("~cat/pkg-4", "cat/pkg"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1"),
            (">cat/pkg-4-r1:0=", "cat/pkg"),
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(atom.key(), key);
        }
    }

    #[test]
    fn test_atom_version() {
        let mut atom: Atom;
        for (s, version) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", Some("<4")),
            ("<=cat/pkg-4-r1", Some("<=4-r1")),
            ("=cat/pkg-4", Some("=4")),
            ("=cat/pkg-4*", Some("=4*")),
            ("~cat/pkg-4", Some("~4")),
            (">=cat/pkg-r1-2-r3", Some(">=2-r3")),
            (">cat/pkg-4-r1:0=", Some(">4-r1")),
        ] {
            atom = Atom::from_str(&s).unwrap();
            let version = version.map(|s| parse::version_with_op(s).unwrap());
            assert_eq!(atom.version(), version.as_ref());
        }
    }

    #[test]
    fn test_atom_revision() {
        let mut atom: Atom;
        for (s, revision) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", Some("0")),
            ("<=cat/pkg-4-r1", Some("1")),
            (">=cat/pkg-r1-2-r3", Some("3")),
            (">cat/pkg-4-r1:0=", Some("1")),
        ] {
            atom = Atom::from_str(&s).unwrap();
            let revision = revision.map(|s| version::Revision::from_str(s).unwrap());
            assert_eq!(atom.revision(), revision.as_ref(), "{s} failed");
        }
    }

    #[test]
    fn test_atom_cpv() {
        let mut atom: Atom;
        for (s, cpv) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg-4"),
            ("<=cat/pkg-4-r1", "cat/pkg-4-r1"),
            ("=cat/pkg-4", "cat/pkg-4"),
            ("=cat/pkg-4*", "cat/pkg-4"),
            ("~cat/pkg-4", "cat/pkg-4"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1-2-r3"),
            (">cat/pkg-4-r1:0=", "cat/pkg-4-r1"),
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(atom.cpv(), cpv);
        }
    }

    #[test]
    fn test_sorting() {
        let atoms = Atoms::load().unwrap();
        for (unsorted, expected) in atoms.sorting() {
            let mut atoms: Vec<_> = unsorted
                .iter()
                .map(|s| Atom::from_str(s).unwrap())
                .collect();
            atoms.sort();
            let sorted: Vec<_> = atoms.iter().map(|x| format!("{x}")).collect();
            assert_eq!(sorted, expected);
        }
    }

    #[test]
    fn test_restrict_methods() {
        let unversioned = Atom::from_str("cat/pkg").unwrap();
        let cpv = cpv("cat/pkg-1").unwrap();
        let full = Atom::from_str("=cat/pkg-1:2/3[u1,u2]::repo").unwrap();

        // category
        let r = Restrict::category("cat");
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // package
        let r = Restrict::package("pkg");
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // no version
        let r = Restrict::version(None).unwrap();
        assert!(r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(!r.matches(&full));

        // version
        let r = Restrict::version(Some("1")).unwrap();
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // no slot
        let r = Restrict::slot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // slot
        let r = Restrict::slot(Some("2"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no subslot
        let r = Restrict::subslot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // subslot
        let r = Restrict::subslot(Some("3"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no use deps specified
        let r = Restrict::use_deps([] as [&str; 0]);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // use deps specified
        for u in [vec!["u1"], vec!["u1", "u2"]] {
            let r = Restrict::use_deps(u);
            assert!(!r.matches(&unversioned));
            assert!(!r.matches(&cpv));
            assert!(r.matches(&full));
        }

        // no repo
        let r = Restrict::repo(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // repo
        let r = Restrict::repo(Some("repo"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));
    }

    #[test]
    fn test_restrict_conversion() {
        let unversioned = Atom::from_str("cat/pkg").unwrap();
        let cpv = cpv("cat/pkg-1").unwrap();
        let full = Atom::from_str("=cat/pkg-1:2/3[u1,u2]::repo").unwrap();

        // unversioned restriction
        let r = BaseRestrict::from(&unversioned);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // cpv restriction
        let r = BaseRestrict::from(&cpv);
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // full atom restriction
        let r = BaseRestrict::from(&full);
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));
    }

    #[test]
    fn test_restrict_versions() {
        let lt = Atom::from_str("<cat/pkg-1-r1").unwrap();
        let le = Atom::from_str("<=cat/pkg-1-r1").unwrap();
        let eq = Atom::from_str("=cat/pkg-1-r1").unwrap();
        let eq_glob = Atom::from_str("=cat/pkg-1*").unwrap();
        let approx = Atom::from_str("~cat/pkg-1").unwrap();
        let ge = Atom::from_str(">=cat/pkg-1-r1").unwrap();
        let gt = Atom::from_str(">cat/pkg-1-r1").unwrap();

        let lt_cpv = cpv("cat/pkg-0").unwrap();
        let gt_cpv = cpv("cat/pkg-2").unwrap();

        let r = BaseRestrict::from(&lt);
        assert!(r.matches(&lt_cpv));
        assert!(!r.matches(&lt));
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&le);
        assert!(r.matches(&lt_cpv));
        assert!(r.matches(&le));
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&eq);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&eq));
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&eq_glob);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&eq_glob));
        for s in ["cat/pkg-1-r1", "cat/pkg-10", "cat/pkg-1.0.1"] {
            let cpv = cpv(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));
        let r = BaseRestrict::from(&approx);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&approx));
        for s in ["cat/pkg-1-r1", "cat/pkg-1-r999"] {
            let cpv = cpv(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&ge);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&ge));
        assert!(r.matches(&gt_cpv));

        let r = BaseRestrict::from(&gt);
        assert!(!r.matches(&lt_cpv));
        assert!(!r.matches(&gt));
        assert!(r.matches(&gt_cpv));
    }
}
