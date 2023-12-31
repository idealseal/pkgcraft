use cached::{proc_macro::cached, SizedCache};

use crate::dep::cpv::{Cpv, CpvOrDep};
use crate::dep::pkg::{Blocker, Dep, Slot, SlotDep, SlotOperator};
use crate::dep::uri::Uri;
use crate::dep::use_dep::{UseDep, UseDepDefault, UseDepKind};
use crate::dep::version::{Number, Operator, Revision, Suffix, SuffixKind, Version, WithOp};
use crate::dep::{Dependency, DependencySet};
use crate::eapi::{Eapi, Feature};
use crate::error::peg_error;
use crate::pkg::ebuild::iuse::Iuse;
use crate::pkg::ebuild::keyword::{Keyword, KeywordStatus};
use crate::traits::IntoOwned;
use crate::types::Ordered;

peg::parser!(grammar depspec() for str {
    // Keywords must not begin with a hyphen.
    rule keyword_name() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-']*
        } / expected!("keyword name"))
        { s }

    // The "-*" keyword is allowed in KEYWORDS for package metadata.
    pub(super) rule keyword() -> Keyword<&'input str>
        = arch:keyword_name() { Keyword { status: KeywordStatus::Stable, arch } }
        / "~" arch:keyword_name() { Keyword { status: KeywordStatus::Unstable, arch } }
        / "-" arch:keyword_name() { Keyword { status: KeywordStatus::Disabled, arch } }
        / "-*" { Keyword { status: KeywordStatus::Disabled, arch: "*" } }

    // License names must not begin with a hyphen, dot, or plus sign.
    pub(super) rule license_name() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        }) { s }

    // Eclass names must not begin with a hyphen or dot and cannot be named "default".
    pub(super) rule eclass_name() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '.' | '-']*
        } / expected!("eclass name")) {?
            if s == "default" {
                Err("eclass cannot be named: default")
            } else {
                Ok(s)
            }
        }

    // Categories must not begin with a hyphen, dot, or plus sign.
    pub(super) rule category() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!("category name"))
        { s }

    // Packages must not begin with a hyphen or plus sign and must not end in a
    // hyphen followed by anything matching a version.
    pub(super) rule package() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            (['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_'] /
                ("-" !(version() ("-" version())? (__ / "*" / ":" / "[" / ![_]))))*
        } / expected!("package name"))
        { s }

    pub(super) rule number() -> Number<&'input str>
        = s:$(['0'..='9']+) {?
            let value = s.parse().map_err(|_| "integer overflow")?;
            Ok(Number { raw: s, value })
        }

    rule suffix() -> SuffixKind
        = "alpha" { SuffixKind::Alpha }
        / "beta" { SuffixKind::Beta }
        / "pre" { SuffixKind::Pre }
        / "rc" { SuffixKind::Rc }
        / "p" { SuffixKind::P }

    rule version_suffix() -> Suffix<&'input str>
        = "_" kind:suffix() version:number()? { Suffix { kind, version } }

    pub(super) rule version() -> Version<&'input str>
        = numbers:number() ++ "." letter:['a'..='z']?
                suffixes:version_suffix()* revision:revision()? {
            Version {
                op: None,
                numbers,
                letter,
                suffixes,
                revision: revision.unwrap_or_default(),
            }
        }

    pub(super) rule version_with_op() -> Version<&'input str>
        = v:with_op(<version()>) { v }

    rule with_op<T: WithOp>(expr: rule<T>) -> T::WithOp
        = "<=" v:expr() {? v.with_op(Operator::LessOrEqual) }
        / "<" v:expr() {? v.with_op(Operator::Less) }
        / ">=" v:expr() {? v.with_op(Operator::GreaterOrEqual) }
        / ">" v:expr() {? v.with_op(Operator::Greater) }
        / "=" v:expr() glob:"*"? {?
            if glob.is_none() {
                v.with_op(Operator::Equal)
            } else {
                v.with_op(Operator::EqualGlob)
            }
        } / "~" v:expr() {? v.with_op(Operator::Approximate) }

    rule revision() -> Revision<&'input str>
        = "-r" rev:number() { Revision(rev) }

    // Slot names must not begin with a hyphen, dot, or plus sign.
    pub(super) rule slot_name() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!("slot name")
        ) { s }

    pub(super) rule slot() -> Slot<&'input str>
        = name:$(slot_name() ("/" slot_name())?)
        { Slot { name } }

    pub(super) rule slot_dep() -> SlotDep<&'input str>
        = "=" { SlotDep { slot: None, op: Some(SlotOperator::Equal) } }
        / "*" { SlotDep { slot: None, op: Some(SlotOperator::Star) } }
        / slot:slot() op:$("=")? {
            let op = op.map(|_| SlotOperator::Equal);
            SlotDep { slot: Some(slot), op }
        }

    rule slot_dep_str() -> SlotDep<&'input str>
        = ":" slot_dep:slot_dep() { slot_dep }

    rule blocker() -> Blocker
        = s:$("!" "!"?) {?
            s.parse().map_err(|_| "invalid blocker")
        }

    pub(super) rule use_flag() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
        } / expected!("USE flag name")
        ) { s }

    pub(super) rule iuse() -> Iuse<&'input str>
        = flag:use_flag() { Iuse { flag, default: None } }
        / "+" flag:use_flag() { Iuse { flag, default: Some(true) } }
        / "-" flag:use_flag() { Iuse { flag, default: Some(false) } }

    rule use_dep_default() -> UseDepDefault
        = "(+)" { UseDepDefault::Enabled }
        / "(-)" { UseDepDefault::Disabled }

    pub(super) rule use_dep() -> UseDep<&'input str>
        = flag:use_flag() default:use_dep_default()? kind:$(['=' | '?'])? {
            let kind = match kind {
                Some("=") => UseDepKind::Equal,
                Some("?") => UseDepKind::EnabledConditional,
                None => UseDepKind::Enabled,
                _ => unreachable!("invalid use dep kind"),
            };
            UseDep { kind, flag, default }
        } / "-" flag:use_flag() default:use_dep_default()? {
            UseDep { kind: UseDepKind::Disabled, flag, default }
        } / "!" flag:use_flag() default:use_dep_default()? kind:$(['=' | '?']) {
            let kind = match kind {
                "=" => UseDepKind::NotEqual,
                "?" => UseDepKind::DisabledConditional,
                _ => unreachable!("invalid use dep kind"),
            };
            UseDep { kind, flag, default }
        } / expected!("use dep")

    rule use_deps() -> Vec<UseDep<&'input str>>
        = "[" use_deps:use_dep() ++ "," "]" { use_deps }

    // repo must not begin with a hyphen and must also be a valid package name
    pub(super) rule repo() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            (['a'..='z' | 'A'..='Z' | '0'..='9' | '_'] / ("-" !version()))*
        } / expected!("repo name")
        ) { s }

    rule repo_dep(eapi: &'static Eapi) -> &'input str
        = "::" repo:repo() {?
            if eapi.has(Feature::RepoIds) {
                Ok(repo)
            } else {
                Err("repo deps aren't supported in official EAPIs")
            }
        }

    pub(super) rule cpv() -> Cpv<&'input str>
        = category:category() "/" package:package() "-" version:version()
        { Cpv { category, package, version } }

    rule dep_pkg() -> Dep<&'input str>
        = dep:cpn() { dep }
        / dep:with_op(<cpv()>) { dep }

    pub(super) rule cpn() -> Dep<&'input str>
        = category:category() "/" package:package() {
            Dep { category, package, ..Default::default() }
        }

    pub(super) rule dep(eapi: &'static Eapi) -> Dep<&'input str>
        = blocker:blocker()? dep:dep_pkg() slot:slot_dep_str()?
                repo:repo_dep(eapi)? use_deps:use_deps()? {
            dep.with(blocker, slot, use_deps, repo)
        }

    pub(super) rule cpv_or_dep() -> CpvOrDep<&'input str>
        = cpv:cpv() { CpvOrDep::Cpv(cpv) }
        / dep:dep(Default::default()) { CpvOrDep::Dep(dep) }

    rule _ = quiet!{[^ ' ' | '\n' | '\t']+}
    rule __ = quiet!{[' ' | '\n' | '\t']+}

    rule parens<T>(expr: rule<T>) -> Vec<T>
        = "(" __ v:expr() ++ __ __ ")" { v }

    rule all_of<T: Ordered>(expr: rule<Dependency<String, T>>) -> Dependency<String, T>
        = vals:parens(<expr()>)
        { Dependency::AllOf(vals.into_iter().map(Box::new).collect()) }

    rule any_of<T: Ordered>(expr: rule<Dependency<String, T>>) -> Dependency<String, T>
        = "||" __ vals:parens(<expr()>)
        { Dependency::AnyOf(vals.into_iter().map(Box::new).collect()) }

    rule conditional<T: Ordered>(expr: rule<Dependency<String, T>>) -> Dependency<String, T>
        = disabled:"!"? flag:use_flag() "?" __ vals:parens(<expr()>) {
            let kind = if disabled.is_none() {
                UseDepKind::EnabledConditional
            } else {
                UseDepKind::DisabledConditional
            };
            let use_dep = UseDep { kind, flag: flag.to_string(), default: None };
            let deps = vals.into_iter().map(Box::new).collect();
            Dependency::Conditional(use_dep, deps)
        }

    rule exactly_one_of<T: Ordered>(expr: rule<Dependency<String, T>>) -> Dependency<String, T>
        = "^^" __ vals:parens(<expr()>)
        { Dependency::ExactlyOneOf(vals.into_iter().map(Box::new).collect()) }

    rule at_most_one_of<T: Ordered>(eapi: &'static Eapi, expr: rule<Dependency<String, T>>) -> Dependency<String, T>
        = "??" __ vals:parens(<expr()>)
        { Dependency::AtMostOneOf(vals.into_iter().map(Box::new).collect()) }

    pub(super) rule license_dependency() -> Dependency<String, String>
        = conditional(<license_dependency()>)
        / any_of(<license_dependency()>)
        / all_of(<license_dependency()>)
        / s:license_name() { Dependency::Enabled(s.to_string()) }

    pub(super) rule src_uri_dependency(eapi: &'static Eapi) -> Dependency<String, Uri>
        = conditional(<src_uri_dependency(eapi)>)
        / all_of(<src_uri_dependency(eapi)>)
        / s:$(quiet!{!")" _+}) rename:(__ "->" __ s:$(_+) {s})? {?
            let uri = Uri::try_new(s, rename).map_err(|_| "invalid URI")?;
            Ok(Dependency::Enabled(uri))
        }

    // Technically RESTRICT tokens have no restrictions, but license
    // restrictions are currently used in order to properly parse use restrictions.
    pub(super) rule properties_dependency() -> Dependency<String, String>
        = conditional(<properties_dependency()>)
        / all_of(<properties_dependency()>)
        / s:license_name() { Dependency::Enabled(s.to_string()) }

    pub(super) rule required_use_dependency(eapi: &'static Eapi) -> Dependency<String, String>
        = conditional(<required_use_dependency(eapi)>)
        / any_of(<required_use_dependency(eapi)>)
        / all_of(<required_use_dependency(eapi)>)
        / exactly_one_of(<required_use_dependency(eapi)>)
        / at_most_one_of(eapi, <required_use_dependency(eapi)>)
        / "!" s:use_flag() { Dependency::Disabled(s.to_string()) }
        / s:use_flag() { Dependency::Enabled(s.to_string()) }

    // Technically RESTRICT tokens have no restrictions, but license
    // restrictions are currently used in order to properly parse use restrictions.
    pub(super) rule restrict_dependency() -> Dependency<String, String>
        = conditional(<restrict_dependency()>)
        / all_of(<restrict_dependency()>)
        / s:license_name() { Dependency::Enabled(s.to_string()) }

    pub(super) rule package_dependency(eapi: &'static Eapi) -> Dependency<String, Dep<String>>
        = conditional(<package_dependency(eapi)>)
        / any_of(<package_dependency(eapi)>)
        / all_of(<package_dependency(eapi)>)
        / dep:dep(eapi) { Dependency::Enabled(dep.into_owned()) }

    pub(super) rule license_dependency_set() -> DependencySet<String, String>
        = v:license_dependency() ** __ { v.into_iter().collect() }

    pub(super) rule src_uri_dependency_set(eapi: &'static Eapi) -> DependencySet<String, Uri>
        = v:src_uri_dependency(eapi) ** __ { v.into_iter().collect() }

    pub(super) rule properties_dependency_set() -> DependencySet<String, String>
        = v:properties_dependency() ** __ { v.into_iter().collect() }

    pub(super) rule required_use_dependency_set(eapi: &'static Eapi) -> DependencySet<String, String>
        = v:required_use_dependency(eapi) ** __ { v.into_iter().collect() }

    pub(super) rule restrict_dependency_set() -> DependencySet<String, String>
        = v:restrict_dependency() ** __ { v.into_iter().collect() }

    pub(super) rule package_dependency_set(eapi: &'static Eapi) -> DependencySet<String, Dep<String>>
        = v:package_dependency(eapi) ** __ { v.into_iter().collect() }
});

pub fn category(s: &str) -> crate::Result<&str> {
    depspec::category(s).map_err(|e| peg_error("invalid category name", s, e))
}

pub fn package(s: &str) -> crate::Result<&str> {
    depspec::package(s).map_err(|e| peg_error("invalid package name", s, e))
}

pub(super) fn version(s: &str) -> crate::Result<Version<&str>> {
    depspec::version(s).map_err(|e| peg_error("invalid version", s, e))
}

pub(super) fn version_with_op(s: &str) -> crate::Result<Version<&str>> {
    depspec::version_with_op(s).map_err(|e| peg_error("invalid version", s, e))
}

pub fn license_name(s: &str) -> crate::Result<&str> {
    depspec::license_name(s).map_err(|e| peg_error("invalid license name", s, e))
}

pub fn eclass_name(s: &str) -> crate::Result<&str> {
    depspec::eclass_name(s).map_err(|e| peg_error("invalid eclass name", s, e))
}

pub fn slot(s: &str) -> crate::Result<Slot<&str>> {
    depspec::slot(s).map_err(|e| peg_error("invalid slot", s, e))
}

pub(super) fn use_dep(s: &str) -> crate::Result<UseDep<&str>> {
    depspec::use_dep(s).map_err(|e| peg_error("invalid use dep", s, e))
}

pub(super) fn slot_dep(s: &str) -> crate::Result<SlotDep<&str>> {
    depspec::slot_dep(s).map_err(|e| peg_error("invalid slot", s, e))
}

pub fn use_flag(s: &str) -> crate::Result<&str> {
    depspec::use_flag(s).map_err(|e| peg_error("invalid USE flag", s, e))
}

pub(crate) fn iuse(s: &str) -> crate::Result<Iuse<&str>> {
    depspec::iuse(s).map_err(|e| peg_error("invalid IUSE", s, e))
}

pub(crate) fn keyword(s: &str) -> crate::Result<Keyword<&str>> {
    depspec::keyword(s).map_err(|e| peg_error("invalid KEYWORD", s, e))
}

pub(crate) fn number(s: &str) -> crate::Result<Number<&str>> {
    depspec::number(s).map_err(|e| peg_error("invalid IUSE", s, e))
}

pub fn repo(s: &str) -> crate::Result<&str> {
    depspec::repo(s).map_err(|e| peg_error("invalid repo name", s, e))
}

/// Parse a string into a [`Cpv`].
pub(super) fn cpv(s: &str) -> crate::Result<Cpv<&str>> {
    depspec::cpv(s).map_err(|e| peg_error("invalid cpv", s, e))
}

/// Parse a string into a [`CpvOrDep`].
pub(super) fn cpv_or_dep(s: &str) -> crate::Result<CpvOrDep<&str>> {
    depspec::cpv_or_dep(s).map_err(|e| peg_error("invalid cpv or dep", s, e))
}

pub(super) fn dep_str<'a>(s: &'a str, eapi: &'static Eapi) -> crate::Result<Dep<&'a str>> {
    depspec::dep(s, eapi).map_err(|e| peg_error("invalid dep", s, e))
}

#[cached(
    type = "SizedCache<(String, &Eapi), crate::Result<Dep<String>>>",
    create = "{ SizedCache::with_size(1000) }",
    convert = r#"{ (s.to_string(), eapi) }"#
)]
pub(crate) fn dep(s: &str, eapi: &'static Eapi) -> crate::Result<Dep<String>> {
    dep_str(s, eapi).into_owned()
}

pub(super) fn cpn(s: &str) -> crate::Result<Dep<&str>> {
    depspec::cpn(s).map_err(|e| peg_error("invalid unversioned dep", s, e))
}

pub fn license_dependency_set(s: &str) -> crate::Result<DependencySet<String, String>> {
    depspec::license_dependency_set(s).map_err(|e| peg_error("invalid LICENSE", s, e))
}

pub fn license_dependency(s: &str) -> crate::Result<Dependency<String, String>> {
    depspec::license_dependency(s).map_err(|e| peg_error("invalid LICENSE dependency", s, e))
}

pub fn src_uri_dependency_set(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<DependencySet<String, Uri>> {
    depspec::src_uri_dependency_set(s, eapi).map_err(|e| peg_error("invalid SRC_URI", s, e))
}

pub fn src_uri_dependency(s: &str, eapi: &'static Eapi) -> crate::Result<Dependency<String, Uri>> {
    depspec::src_uri_dependency(s, eapi).map_err(|e| peg_error("invalid SRC_URI dependency", s, e))
}

pub fn properties_dependency_set(s: &str) -> crate::Result<DependencySet<String, String>> {
    depspec::properties_dependency_set(s).map_err(|e| peg_error("invalid PROPERTIES", s, e))
}

pub fn properties_dependency(s: &str) -> crate::Result<Dependency<String, String>> {
    depspec::properties_dependency(s).map_err(|e| peg_error("invalid PROPERTIES dependency", s, e))
}

pub fn required_use_dependency_set(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<DependencySet<String, String>> {
    depspec::required_use_dependency_set(s, eapi)
        .map_err(|e| peg_error("invalid REQUIRED_USE", s, e))
}

pub fn required_use_dependency(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<Dependency<String, String>> {
    depspec::required_use_dependency(s, eapi)
        .map_err(|e| peg_error("invalid REQUIRED_USE dependency", s, e))
}

pub fn restrict_dependency_set(s: &str) -> crate::Result<DependencySet<String, String>> {
    depspec::restrict_dependency_set(s).map_err(|e| peg_error("invalid RESTRICT", s, e))
}

pub fn restrict_dependency(s: &str) -> crate::Result<Dependency<String, String>> {
    depspec::restrict_dependency(s).map_err(|e| peg_error("invalid RESTRICT dependency", s, e))
}

pub fn package_dependency_set(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<DependencySet<String, Dep<String>>> {
    depspec::package_dependency_set(s, eapi).map_err(|e| peg_error("invalid dependency", s, e))
}

pub fn package_dependency(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<Dependency<String, Dep<String>>> {
    depspec::package_dependency(s, eapi).map_err(|e| peg_error("invalid package dependency", s, e))
}

#[cfg(test)]
mod tests {
    use crate::eapi::{EAPIS, EAPIS_OFFICIAL, EAPI_LATEST_OFFICIAL, EAPI_PKGCRAFT};

    use super::*;

    #[test]
    fn slots() {
        for slot in ["0", "a", "_", "_a", "99", "aBc", "a+b_c.d-e"] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg:{slot}");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                assert_eq!(d.slot(), Some(slot));
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn blockers() {
        let d = dep("cat/pkg", &EAPI_LATEST_OFFICIAL).unwrap();
        assert!(d.blocker().is_none());

        for (s, blocker) in [
            ("!cat/pkg", Some(Blocker::Weak)),
            ("!cat/pkg:0", Some(Blocker::Weak)),
            ("!!cat/pkg", Some(Blocker::Strong)),
            ("!!<cat/pkg-1", Some(Blocker::Strong)),
        ] {
            for eapi in &*EAPIS {
                let result = dep(s, eapi);
                assert!(result.is_ok(), "{s:?} failed for EAPI {eapi}: {}", result.err().unwrap());
                let d = result.unwrap();
                assert_eq!(d.blocker(), blocker);
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn use_deps() {
        for use_deps in ["a", "!a?", "a,b", "-a,-b", "a?,b?", "a,b=,!c=,d?,!e?,-f"] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg[{use_deps}]");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                let expected = use_deps.parse().unwrap();
                assert_eq!(d.use_deps(), Some(&expected));
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn use_dep_defaults() {
        for use_deps in ["a(+)", "-a(-)", "a(+)?,!b(-)?", "a(-)=,!b(+)="] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg[{use_deps}]");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                let expected = use_deps.parse().unwrap();
                assert_eq!(d.use_deps(), Some(&expected));
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn subslots() {
        for (slot_str, slot, subslot, slot_op) in [
            ("0/1", Some("0"), Some("1"), None),
            ("a/b", Some("a"), Some("b"), None),
            ("A/B", Some("A"), Some("B"), None),
            ("_/_", Some("_"), Some("_"), None),
            ("0/a.b+c-d_e", Some("0"), Some("a.b+c-d_e"), None),
        ] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg:{slot_str}");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                assert_eq!(d.slot(), slot);
                assert_eq!(d.subslot(), subslot);
                assert_eq!(d.slot_op(), slot_op);
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn slot_ops() {
        for (slot_str, slot, subslot, slot_op) in [
            ("*", None, None, Some(SlotOperator::Star)),
            ("=", None, None, Some(SlotOperator::Equal)),
            ("0=", Some("0"), None, Some(SlotOperator::Equal)),
            ("a=", Some("a"), None, Some(SlotOperator::Equal)),
            ("0/1=", Some("0"), Some("1"), Some(SlotOperator::Equal)),
            ("a/b=", Some("a"), Some("b"), Some(SlotOperator::Equal)),
        ] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg:{slot_str}");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                assert_eq!(d.slot(), slot);
                assert_eq!(d.subslot(), subslot);
                assert_eq!(d.slot_op(), slot_op);
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn repos() {
        for repo in ["_", "a", "repo", "repo_a", "repo-a"] {
            let s = format!("cat/pkg::{repo}");

            // repo ids aren't supported in official EAPIs
            for eapi in &*EAPIS_OFFICIAL {
                assert!(dep(&s, eapi).is_err(), "{s:?} didn't fail");
            }

            let result = dep(&s, &EAPI_PKGCRAFT);
            assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
            let d = result.unwrap();
            assert_eq!(d.repo(), Some(repo));
            assert_eq!(d.to_string(), s);
        }
    }

    #[test]
    fn license() {
        // invalid
        for s in ["(", ")", "( )", "( l1)", "| ( l1 )", "!use ( l1 )"] {
            assert!(license_dependency_set(s).is_err(), "{s:?} didn't fail");
            assert!(license_dependency(s).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(license_dependency_set("").unwrap().is_empty());

        // valid
        for (s, expected_flatten) in [
            // simple values
            ("v", vec!["v"]),
            ("v1 v2", vec!["v1", "v2"]),
            // groupings
            ("( v )", vec!["v"]),
            ("( v1 v2 )", vec!["v1", "v2"]),
            ("( v1 ( v2 ) )", vec!["v1", "v2"]),
            ("( ( v ) )", vec!["v"]),
            ("|| ( v )", vec!["v"]),
            ("|| ( v1 v2 )", vec!["v1", "v2"]),
            // conditionals
            ("u? ( v )", vec!["v"]),
            ("u? ( v1 v2 )", vec!["v1", "v2"]),
            // combinations
            ("v1 u? ( v2 )", vec!["v1", "v2"]),
            ("!u? ( || ( v1 v2 ) )", vec!["v1", "v2"]),
        ] {
            let depset = license_dependency_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn src_uri() {
        // invalid
        for s in ["http://", "https://a/uri/with/no/filename/"] {
            assert!(src_uri_dependency_set(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
            assert!(src_uri_dependency(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(src_uri_dependency_set("", &EAPI_LATEST_OFFICIAL)
            .unwrap()
            .is_empty());

        // valid
        for (s, expected_flatten) in [
            ("uri", vec!["uri"]),
            ("http://uri", vec!["http://uri"]),
            ("uri1 uri2", vec!["uri1", "uri2"]),
            ("( http://uri1 http://uri2 )", vec!["http://uri1", "http://uri2"]),
            ("u1? ( http://uri1 !u2? ( http://uri2 ) )", vec!["http://uri1", "http://uri2"]),
        ] {
            for eapi in &*EAPIS {
                let depset = src_uri_dependency_set(s, eapi).unwrap();
                assert_eq!(depset.to_string(), s);
                let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
                assert_eq!(flatten, expected_flatten);
            }
        }

        // renames
        for (s, expected_flatten) in [
            ("http://uri -> file", vec!["http://uri -> file"]),
            ("u? ( http://uri -> file )", vec!["http://uri -> file"]),
        ] {
            for eapi in &*EAPIS {
                let depset = src_uri_dependency_set(s, eapi).unwrap();
                assert_eq!(depset.to_string(), s);
                let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
                assert_eq!(flatten, expected_flatten);
            }
        }
    }

    #[test]
    fn required_use() {
        // invalid
        for s in ["(", ")", "( )", "( u)", "| ( u )", "|| ( )", "^^ ( )", "?? ( )"] {
            assert!(
                required_use_dependency_set(s, &EAPI_LATEST_OFFICIAL).is_err(),
                "{s:?} didn't fail"
            );
            assert!(
                required_use_dependency(s, &EAPI_LATEST_OFFICIAL).is_err(),
                "{s:?} didn't fail"
            );
        }

        // empty set
        assert!(required_use_dependency_set("", &EAPI_LATEST_OFFICIAL)
            .unwrap()
            .is_empty());

        // valid
        for (s, expected_flatten) in [
            ("u", vec!["u"]),
            ("!u", vec!["u"]),
            ("u1 !u2", vec!["u1", "u2"]),
            ("( u )", vec!["u"]),
            ("( u1 u2 )", vec!["u1", "u2"]),
            ("|| ( u )", vec!["u"]),
            ("|| ( !u1 u2 )", vec!["u1", "u2"]),
            ("^^ ( u1 !u2 )", vec!["u1", "u2"]),
            ("u1? ( u2 )", vec!["u2"]),
            ("u1? ( u2 !u3 )", vec!["u2", "u3"]),
            ("!u1? ( || ( u2 u3 ) )", vec!["u2", "u3"]),
        ] {
            let depset = required_use_dependency_set(s, &EAPI_LATEST_OFFICIAL).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }

        // ?? operator
        for (s, expected_flatten) in [("?? ( u1 u2 )", vec!["u1", "u2"])] {
            for eapi in &*EAPIS {
                let depset = required_use_dependency_set(s, eapi).unwrap();
                assert_eq!(depset.to_string(), s);
                let flatten: Vec<_> = depset.iter_flatten().collect();
                assert_eq!(flatten, expected_flatten);
            }
        }
    }

    #[test]
    fn package() {
        // invalid
        for s in ["(", ")", "( )", "|| ( )", "( a/b)", "| ( a/b )", "use ( a/b )", "!use ( a/b )"] {
            assert!(package_dependency_set(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
            assert!(package_dependency(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(package_dependency_set("", &EAPI_LATEST_OFFICIAL)
            .unwrap()
            .is_empty());

        // valid
        for (s, expected_flatten) in [
            ("a/b", vec!["a/b"]),
            ("a/b c/d", vec!["a/b", "c/d"]),
            ("( a/b c/d )", vec!["a/b", "c/d"]),
            ("u? ( a/b c/d )", vec!["a/b", "c/d"]),
            ("!u? ( a/b c/d )", vec!["a/b", "c/d"]),
            ("u1? ( a/b !u2? ( c/d ) )", vec!["a/b", "c/d"]),
        ] {
            let depset = package_dependency_set(s, &EAPI_LATEST_OFFICIAL).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn properties() {
        // invalid
        for s in ["(", ")", "( )", "( v)", "| ( v )", "!use ( v )", "|| ( v )", "|| ( v1 v2 )"] {
            assert!(properties_dependency_set(s).is_err(), "{s:?} didn't fail");
            assert!(properties_dependency(s).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(properties_dependency_set("").unwrap().is_empty());

        // valid
        for (s, expected_flatten) in [
            // simple values
            ("v", vec!["v"]),
            ("v1 v2", vec!["v1", "v2"]),
            // groupings
            ("( v )", vec!["v"]),
            ("( v1 v2 )", vec!["v1", "v2"]),
            ("( v1 ( v2 ) )", vec!["v1", "v2"]),
            ("( ( v ) )", vec!["v"]),
            // conditionals
            ("u? ( v )", vec!["v"]),
            ("u? ( v1 v2 )", vec!["v1", "v2"]),
            ("!u? ( v1 v2 )", vec!["v1", "v2"]),
            // combinations
            ("v1 u? ( v2 )", vec!["v1", "v2"]),
        ] {
            let depset = properties_dependency_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn restrict() {
        // invalid
        for s in ["(", ")", "( )", "( v)", "| ( v )", "!use ( v )", "|| ( v )", "|| ( v1 v2 )"] {
            assert!(restrict_dependency_set(s).is_err(), "{s:?} didn't fail");
            assert!(restrict_dependency(s).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(restrict_dependency_set("").unwrap().is_empty());

        // valid
        for (s, expected_flatten) in [
            // simple values
            ("v", vec!["v"]),
            ("v1 v2", vec!["v1", "v2"]),
            // groupings
            ("( v )", vec!["v"]),
            ("( v1 v2 )", vec!["v1", "v2"]),
            ("( v1 ( v2 ) )", vec!["v1", "v2"]),
            ("( ( v ) )", vec!["v"]),
            // conditionals
            ("u? ( v )", vec!["v"]),
            ("u? ( v1 v2 )", vec!["v1", "v2"]),
            ("!u? ( v1 v2 )", vec!["v1", "v2"]),
            // combinations
            ("v1 u? ( v2 )", vec!["v1", "v2"]),
        ] {
            let depset = restrict_dependency_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }
}
