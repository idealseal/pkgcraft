use std::collections::HashMap;
use std::io::{self, Write};
use std::{fmt, fs};

use itertools::Itertools;
use scallop::{functions, variables};
use strum::{AsRefStr, Display, EnumString};
use tracing::warn;

use crate::dep::{self, Cpv, Dep, DepSet, Slot, Uri};
use crate::eapi::Eapi;
use crate::files::atomic_write_file;
use crate::pkg::{ebuild::raw::Pkg, Package, RepoPackage, Source};
use crate::repo::ebuild::Repo;
use crate::traits::IntoOwned;
use crate::types::OrderedSet;
use crate::Error;

use super::{get_build_mut, BuildData};

#[derive(
    AsRefStr, EnumString, Display, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
#[strum(serialize_all = "UPPERCASE")]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub enum Key {
    BDEPEND,
    DEFINED_PHASES,
    DEPEND,
    DESCRIPTION,
    EAPI,
    HOMEPAGE,
    IDEPEND,
    INHERIT,
    IUSE,
    KEYWORDS,
    LICENSE,
    PDEPEND,
    PROPERTIES,
    RDEPEND,
    REQUIRED_USE,
    RESTRICT,
    SLOT,
    SRC_URI,
    // match ordering of previous implementations (although the cache format is unordered)
    INHERITED,
    CHKSUM,
}

impl Key {
    pub(crate) fn get(&self, build: &mut BuildData, eapi: &'static Eapi) -> Option<String> {
        use Key::*;
        match self {
            CHKSUM => None,
            DEFINED_PHASES => {
                let mut phase_names: Vec<_> = eapi
                    .phases()
                    .iter()
                    .filter_map(|p| functions::find(p).map(|_| p.short_name()))
                    .collect();
                if phase_names.is_empty() {
                    None
                } else {
                    phase_names.sort_unstable();
                    Some(phase_names.join(" "))
                }
            }
            INHERIT => {
                let eclasses = &build.inherit;
                if eclasses.is_empty() {
                    None
                } else {
                    Some(eclasses.iter().join(" "))
                }
            }
            INHERITED => {
                let eclasses = &build.inherited;
                if eclasses.is_empty() {
                    None
                } else {
                    Some(eclasses.iter().join(" "))
                }
            }
            key => {
                if let Some(vals) = build.incrementals.get(key) {
                    if vals.is_empty() {
                        None
                    } else {
                        Some(vals.iter().join(" "))
                    }
                } else {
                    variables::optional(key).map(|s| s.split_whitespace().join(" "))
                }
            }
        }
    }
}

/// Package IUSE.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Iuse {
    full: String,
    default: Option<bool>,
}

impl PartialEq<str> for Iuse {
    fn eq(&self, other: &str) -> bool {
        self.full == other
    }
}

impl PartialEq<Iuse> for str {
    fn eq(&self, other: &Iuse) -> bool {
        self == other.full
    }
}

impl fmt::Display for Iuse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.full)
    }
}

impl AsRef<str> for Iuse {
    fn as_ref(&self) -> &str {
        &self.full
    }
}

impl Iuse {
    fn new(s: &str) -> crate::Result<Self> {
        let (default, _flag) = dep::parse::iuse(s)?;
        Ok(Self {
            full: s.to_string(),
            default: default.map(|c| c == '+'),
        })
    }

    /// Return an IUSE flag stripping defaults.
    pub fn flag(&self) -> &str {
        if self.default.is_none() {
            &self.full
        } else {
            &self.full[1..]
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Metadata {
    description: String,
    slot: Slot<String>,
    bdepend: DepSet<String, Dep>,
    depend: DepSet<String, Dep>,
    idepend: DepSet<String, Dep>,
    pdepend: DepSet<String, Dep>,
    rdepend: DepSet<String, Dep>,
    license: DepSet<String, String>,
    properties: DepSet<String, String>,
    required_use: DepSet<String, String>,
    restrict: DepSet<String, String>,
    src_uri: DepSet<String, Uri>,
    homepage: OrderedSet<String>,
    defined_phases: OrderedSet<String>,
    keywords: OrderedSet<String>,
    iuse: OrderedSet<Iuse>,
    inherit: OrderedSet<String>,
    inherited: OrderedSet<String>,
    chksum: String,
}

macro_rules! split {
    ($s:expr) => {
        $s.split_whitespace().map(String::from)
    };
}

impl Metadata {
    /// Convert raw metadata key value to stored value.
    fn convert(&mut self, eapi: &'static Eapi, key: Key, val: &str) -> crate::Result<()> {
        use Key::*;
        match key {
            CHKSUM => self.chksum = val.to_string(),
            DESCRIPTION => self.description = val.to_string(),
            SLOT => self.slot = dep::parse::slot(val)?.into_owned(),
            BDEPEND => self.bdepend = dep::parse::dependencies_dep_set(val, eapi)?,
            DEPEND => self.depend = dep::parse::dependencies_dep_set(val, eapi)?,
            IDEPEND => self.idepend = dep::parse::dependencies_dep_set(val, eapi)?,
            PDEPEND => self.pdepend = dep::parse::dependencies_dep_set(val, eapi)?,
            RDEPEND => self.rdepend = dep::parse::dependencies_dep_set(val, eapi)?,
            LICENSE => self.license = dep::parse::license_dep_set(val)?,
            PROPERTIES => self.properties = dep::parse::properties_dep_set(val)?,
            REQUIRED_USE => self.required_use = dep::parse::required_use_dep_set(val, eapi)?,
            RESTRICT => self.restrict = dep::parse::restrict_dep_set(val)?,
            SRC_URI => self.src_uri = dep::parse::src_uri_dep_set(val, eapi)?,
            HOMEPAGE => self.homepage = split!(val).collect(),
            DEFINED_PHASES => self.defined_phases = split!(val).sorted().collect(),
            KEYWORDS => self.keywords = split!(val).collect(),
            IUSE => {
                self.iuse = val
                    .split_whitespace()
                    .map(Iuse::new)
                    .collect::<crate::Result<OrderedSet<_>>>()?
            }
            INHERIT => self.inherit = split!(val).collect(),
            INHERITED => self.inherited = split!(val).collect(),
            EAPI => {
                let sourced: &Eapi = val.try_into()?;
                if sourced != eapi {
                    return Err(Error::InvalidValue(format!(
                        "mismatched sourced and parsed EAPIs: {sourced} != {eapi}"
                    )));
                }
            }
        }

        Ok(())
    }

    /// Serialize [`Metadata`] to the given package's metadata/md5-cache file in the related repo.
    pub(crate) fn serialize(pkg: &Pkg) -> crate::Result<()> {
        // convert raw pkg into metadata via sourcing
        let meta: Metadata = pkg.try_into()?;
        let eapi = pkg.eapi();

        // return the MD5 checksum for a known eclass
        let eclass_chksum = |name: &str| -> &str {
            pkg.repo()
                .eclasses()
                .get(name)
                .expect("missing eclass")
                .chksum()
        };

        // convert metadata fields to metadata lines
        use Key::*;
        let mut data = vec![];
        for key in eapi.metadata_keys() {
            match key {
                CHKSUM => writeln!(&mut data, "_md5_={}", pkg.chksum())?,
                DESCRIPTION => writeln!(&mut data, "{key}={}", meta.description)?,
                SLOT => writeln!(&mut data, "{key}={}", meta.slot)?,
                BDEPEND => {
                    if !meta.bdepend.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.bdepend)?;
                    }
                }
                DEPEND => {
                    if !meta.depend.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.depend)?;
                    }
                }
                IDEPEND => {
                    if !meta.idepend.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.idepend)?;
                    }
                }
                PDEPEND => {
                    if !meta.pdepend.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.pdepend)?;
                    }
                }
                RDEPEND => {
                    if !meta.rdepend.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.rdepend)?;
                    }
                }
                LICENSE => {
                    if !meta.license.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.license)?;
                    }
                }
                PROPERTIES => {
                    if !meta.properties.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.properties)?;
                    }
                }
                REQUIRED_USE => {
                    if !meta.required_use.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.required_use)?;
                    }
                }
                RESTRICT => {
                    if !meta.restrict.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.restrict)?;
                    }
                }
                SRC_URI => {
                    if !meta.src_uri.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.src_uri)?;
                    }
                }
                HOMEPAGE => {
                    if !meta.homepage.is_empty() {
                        let val = meta.homepage.iter().join(" ");
                        writeln!(&mut data, "{key}={val}")?;
                    }
                }
                DEFINED_PHASES => {
                    // PMS specifies if no phase functions are defined, a single hyphen is used.
                    if meta.defined_phases.is_empty() {
                        writeln!(&mut data, "{key}=-")?;
                    } else {
                        let val = meta.defined_phases.iter().join(" ");
                        writeln!(&mut data, "{key}={val}")?;
                    }
                }
                KEYWORDS => {
                    if !meta.keywords().is_empty() {
                        let val = meta.keywords.iter().join(" ");
                        writeln!(&mut data, "{key}={val}")?;
                    }
                }
                IUSE => {
                    if !meta.iuse().is_empty() {
                        let val = meta.iuse.iter().join(" ");
                        writeln!(&mut data, "{key}={val}")?;
                    }
                }
                INHERIT => {
                    if !meta.inherit().is_empty() {
                        let val = meta.inherit.iter().join(" ");
                        writeln!(&mut data, "{key}={val}")?;
                    }
                }
                INHERITED => {
                    if !meta.inherited.is_empty() {
                        let val = meta
                            .inherited
                            .iter()
                            .flat_map(|s| [s.as_str(), eclass_chksum(s)])
                            .join("\t");
                        writeln!(&mut data, "_eclasses_={val}")?;
                    }
                }
                EAPI => writeln!(&mut data, "{key}={eapi}")?,
            }
        }

        // determine metadata entry directory
        let dir = pkg
            .repo()
            .metadata()
            .cache_path()
            .join(pkg.cpv().category());

        // create metadata entry directory
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .map_err(|e| Error::IO(format!("failed creating metadata dir: {dir}: {e}")))?;
        }

        // atomically create metadata file
        let pf = pkg.pf();
        let path = dir.join(format!(".{pf}"));
        let new_path = dir.join(pf);
        atomic_write_file(&path, data, &new_path)
    }

    /// Verify a metadata entry is valid.
    pub(crate) fn verify(cpv: &Cpv, repo: &Repo) -> bool {
        Pkg::new(cpv.clone(), repo)
            .map(|p| Self::load(&p, false).is_err())
            .unwrap_or_default()
    }

    /// Deserialize a metadata entry for a given package into [`Metadata`].
    pub(crate) fn load(pkg: &Pkg, deserialize: bool) -> crate::Result<Self> {
        let eapi = pkg.eapi();
        let repo = pkg.repo();

        let path = repo.metadata().cache_path().join(pkg.cpv().to_string());
        let data = fs::read_to_string(&path).map_err(|e| {
            if e.kind() != io::ErrorKind::NotFound {
                warn!("error loading ebuild metadata: {path:?}: {e}");
            }
            Error::IO(format!("failed loading ebuild metadata: {path:?}: {e}"))
        })?;

        let mut data: HashMap<_, _> = data
            .lines()
            .filter_map(|l| {
                l.split_once('=').map(|(s, v)| match (s, v) {
                    ("_eclasses_", v) => ("INHERITED", v),
                    ("_md5_", v) => ("CHKSUM", v),
                    // single hyphen means no phases are defined as per PMS
                    ("DEFINED_PHASES", "-") => ("DEFINED_PHASES", ""),
                    _ => (s, v),
                })
            })
            .filter_map(|(k, v)| k.parse().ok().map(|k| (k, v)))
            .filter(|(k, _)| eapi.metadata_keys().contains(k))
            .collect();

        // verify ebuild hash
        if let Some(val) = data.get(&Key::CHKSUM) {
            if *val != pkg.chksum() {
                return Err(Error::InvalidValue("mismatched ebuild checksum".to_string()));
            }
        } else {
            return Err(Error::InvalidValue("missing ebuild checksum".to_string()));
        }

        let mut meta = Self::default();

        // verify eclass hashes
        if let Some(val) = data.remove(&Key::INHERITED) {
            for (name, chksum) in val.split_whitespace().tuples() {
                if !repo
                    .eclasses()
                    .get(name)
                    .map_or(false, |e| e.chksum() == chksum)
                {
                    return Err(Error::InvalidValue("mismatched eclass checksum".to_string()));
                }

                if deserialize {
                    meta.inherited.insert(name.to_string());
                }
            }
        }

        // deserialize values into metadata fields
        if deserialize {
            for (key, val) in data {
                meta.convert(eapi, key, val)?;
            }
        }

        Ok(meta)
    }

    pub(crate) fn description(&self) -> &str {
        &self.description
    }

    pub(crate) fn slot(&self) -> &Slot<String> {
        &self.slot
    }

    pub(crate) fn bdepend(&self) -> &DepSet<String, Dep> {
        &self.bdepend
    }

    pub(crate) fn depend(&self) -> &DepSet<String, Dep> {
        &self.depend
    }

    pub(crate) fn idepend(&self) -> &DepSet<String, Dep> {
        &self.idepend
    }

    pub(crate) fn pdepend(&self) -> &DepSet<String, Dep> {
        &self.pdepend
    }

    pub(crate) fn rdepend(&self) -> &DepSet<String, Dep> {
        &self.rdepend
    }

    pub(crate) fn license(&self) -> &DepSet<String, String> {
        &self.license
    }

    pub(crate) fn properties(&self) -> &DepSet<String, String> {
        &self.properties
    }

    pub(crate) fn required_use(&self) -> &DepSet<String, String> {
        &self.required_use
    }

    pub(crate) fn restrict(&self) -> &DepSet<String, String> {
        &self.restrict
    }

    pub(crate) fn src_uri(&self) -> &DepSet<String, Uri> {
        &self.src_uri
    }

    pub(crate) fn homepage(&self) -> &OrderedSet<String> {
        &self.homepage
    }

    pub(crate) fn defined_phases(&self) -> &OrderedSet<String> {
        &self.defined_phases
    }

    pub(crate) fn keywords(&self) -> &OrderedSet<String> {
        &self.keywords
    }

    pub(crate) fn iuse(&self) -> &OrderedSet<Iuse> {
        &self.iuse
    }

    pub(crate) fn inherit(&self) -> &OrderedSet<String> {
        &self.inherit
    }

    pub(crate) fn inherited(&self) -> &OrderedSet<String> {
        &self.inherited
    }

    pub(crate) fn chksum(&self) -> &str {
        &self.chksum
    }
}

impl TryFrom<&Pkg<'_>> for Metadata {
    type Error = Error;

    fn try_from(pkg: &Pkg) -> crate::Result<Self> {
        // TODO: run sourcing via an external process pool returning the requested variables
        pkg.source()?;

        let eapi = pkg.eapi();
        let build = get_build_mut();
        let mut meta = Self::default();

        // pull metadata values from shell variables
        let mut missing = vec![];
        for key in eapi.metadata_keys() {
            if let Some(val) = key.get(build, eapi) {
                meta.convert(eapi, *key, &val)?;
            } else if eapi.mandatory_keys().contains(key) {
                missing.push(key.as_ref());
            }
        }

        if !missing.is_empty() {
            missing.sort();
            let keys = missing.join(", ");
            return Err(Error::InvalidValue(format!("missing required values: {keys}")));
        }

        Ok(meta)
    }
}
