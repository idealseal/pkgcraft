use peg;

use super::version::ParsedVersion;
use super::{Blocker, ParsedAtom};
use crate::eapi::{Eapi, Feature};

peg::parser! {
    pub(crate) grammar pkg() for str {
        // Categories must not begin with a hyphen, dot, or plus sign.
        pub(super) rule category() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
            } / expected!("category name")
            ) { s }

        // Packages must not begin with a hyphen or plus sign and must not end in a
        // hyphen followed by anything matching a version.
        pub(super) rule package() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                (['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_'] /
                 ("-" !(version() ("-" version())? ![_])))*
            } / expected!("package name")
            ) { s }

        rule version_suffix() -> (&'input str, Option<&'input str>)
            = suffix:$("alpha" / "beta" / "pre" / "rc" / "p") ver:$(['0'..='9']+)? {?
                Ok((suffix, ver))
            }

        // TODO: figure out how to return string slice instead of positions
        // Related issue: https://github.com/kevinmehall/rust-peg/issues/283
        pub(super) rule version() -> ParsedVersion<'input>
            = start:position!() numbers:$(['0'..='9']+) ++ "." letter:['a'..='z']?
                    suffixes:("_" s:version_suffix() ++ "_" {s})?
                    end_base:position!() revision:revision()? end:position!() {
                ParsedVersion {
                    start,
                    end_base,
                    end,
                    numbers,
                    letter,
                    suffixes,
                    revision,
                    ..Default::default()
                }
            }

        pub(super) rule version_with_op() -> ParsedVersion<'input>
            = op:$(("<" "="?) / "=" / "~" / (">" "="?))
                    start:position!() numbers:$(['0'..='9']+) ++ "." letter:['a'..='z']?
                    suffixes:("_" s:version_suffix() ++ "_" {s})?
                    end_base:position!() revision:revision()? end:position!() glob:$("*")? {?
                let ver = ParsedVersion {
                    start,
                    end_base,
                    end,
                    numbers,
                    letter,
                    suffixes,
                    revision,
                    ..Default::default()
                };
                ver.with_op(op, glob)
            }

        rule revision() -> &'input str
            = "-r" s:$(quiet!{['0'..='9']+} / expected!("revision"))
            { s }

        // Slot names must not begin with a hyphen, dot, or plus sign.
        rule slot_name() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
            } / expected!("slot name")
            ) { s }

        rule slot(eapi: &'static Eapi) -> (&'input str, Option<&'input str>)
            = slot:slot_name() subslot:subslot(eapi)? {
                (slot, subslot)
            }

        rule slot_str(eapi: &'static Eapi) -> (Option<&'input str>, Option<&'input str>, Option<&'input str>)
            = op:$("*" / "=") {?
                if !eapi.has(Feature::SlotOps) {
                    return Err("slot operators are supported in >= EAPI 5");
                }
                Ok((None, None, Some(op)))
            } / slot:slot(eapi) op:$("=")? {?
                if op.is_some() && !eapi.has(Feature::SlotOps) {
                    return Err("slot operators are supported in >= EAPI 5");
                }
                Ok((Some(slot.0), slot.1, op))
            }

        rule slot_dep(eapi: &'static Eapi) -> (Option<&'input str>, Option<&'input str>, Option<&'input str>)
            = ":" slot_parts:slot_str(eapi) {?
                if !eapi.has(Feature::SlotDeps) {
                    return Err("slot deps are supported in >= EAPI 1");
                }
                Ok(slot_parts)
            }

        rule blocks(eapi: &'static Eapi) -> Blocker
            = blocks:("!"*<1,2>) {?
                if eapi.has(Feature::Blockers) {
                    match blocks.len() {
                        1 => Ok(Blocker::Weak),
                        2 => Ok(Blocker::Strong),
                        _ => Err("invalid blocker"),
                    }
                } else {
                    Err("blockers are supported in >= EAPI 2")
                }
            }

        rule useflag() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
            } / expected!("useflag name")
            ) { s }

        rule use_dep(eapi: &'static Eapi) -> &'input str
            = s:$(quiet!{
                (useflag() use_dep_default(eapi)? ['=' | '?']?) /
                ("-" useflag() use_dep_default(eapi)?) /
                ("!" useflag() use_dep_default(eapi)? ['=' | '?'])
            } / expected!("use dep")
            ) { s }

        rule use_deps(eapi: &'static Eapi) -> Vec<&'input str>
            = "[" use_deps:use_dep(eapi) ++ "," "]" {?
                if eapi.has(Feature::UseDeps) {
                    Ok(use_deps)
                } else {
                    Err("use deps are supported in >= EAPI 2")
                }
            }

        rule use_dep_default(eapi: &'static Eapi) -> &'input str
            = s:$("(+)" / "(-)") {?
                if eapi.has(Feature::UseDepDefaults) {
                    Ok(s)
                } else {
                    Err("use dep defaults are supported in >= EAPI 4")
                }
            }

        rule subslot(eapi: &'static Eapi) -> &'input str
            = "/" s:slot_name() {?
                if eapi.has(Feature::Subslots) {
                    Ok(s)
                } else {
                    Err("subslots are supported in >= EAPI 5")
                }
            }

        // repo must not begin with a hyphen and must also be a valid package name
        pub(super) rule repo() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                (['a'..='z' | 'A'..='Z' | '0'..='9' | '_'] / ("-" !version()))*
            } / expected!("repo name")
            ) { s }

        rule repo_dep(eapi: &'static Eapi) -> &'input str
            = "::" repo:repo() {?
                if !eapi.has(Feature::RepoIds) {
                    return Err("repo deps aren't supported in EAPIs");
                }
                Ok(repo)
            }

        pub(super) rule cpv() -> ParsedAtom<'input>
            = cat:category() "/" pkg:package() "-" ver:version() {
                ParsedAtom {
                    category: cat,
                    package: pkg,
                    version: Some(ver),
                    ..Default::default()
                }
            }

        pub(super) rule cpv_or_cp() -> (bool, &'input str, &'input str, Option<&'input str>)
            = op:$(("<" "="?) / "=" / "~" / (">" "="?)) cpv:$([^'*']+) glob:$("*")? {
                (true, op, cpv, glob)
            } / cat:category() "/" pkg:package() {
                (false, cat, pkg, None)
            }

        pub(super) rule dep(eapi: &'static Eapi) -> (&'input str, ParsedAtom<'input>)
            = block:blocks(eapi)? dep:$([^':' | '[']+) slot_dep:slot_dep(eapi)?
                    use_deps:use_deps(eapi)? repo:repo_dep(eapi)? {
                let (slot, subslot, slot_op) = slot_dep.unwrap_or_default();
                (dep, ParsedAtom {
                    block,
                    slot,
                    subslot,
                    slot_op,
                    use_deps,
                    repo,
                    ..Default::default()
                })
            }
    }
}

// provide public parsing functionality while converting error types
pub mod parse {
    use cached::{proc_macro::cached, SizedCache};

    use crate::atom::{Atom, Version};
    use crate::peg::peg_error;
    use crate::{Error, Result};

    use super::*;

    pub fn category(s: &str) -> Result<&str> {
        pkg::category(s).map_err(|e| peg_error(format!("invalid category name: {s:?}"), s, e))
    }

    pub fn package(s: &str) -> Result<&str> {
        pkg::package(s).map_err(|e| peg_error(format!("invalid package name: {s:?}"), s, e))
    }

    pub(crate) fn version_str(s: &str) -> Result<ParsedVersion> {
        pkg::version(s).map_err(|e| peg_error(format!("invalid version: {s:?}"), s, e))
    }

    #[cached(
        type = "SizedCache<String, Result<Version>>",
        create = "{ SizedCache::with_size(1000) }",
        convert = r#"{ s.to_string() }"#
    )]
    pub fn version(s: &str) -> Result<Version> {
        let version = version_str(s)?;
        version.into_owned(s)
    }

    pub fn version_with_op(s: &str) -> Result<Version> {
        let parsed_version = pkg::version_with_op(s)
            .map_err(|e| peg_error(format!("invalid version: {s:?}"), s, e))?;
        parsed_version.into_owned(s)
    }

    pub fn repo(s: &str) -> Result<&str> {
        pkg::repo(s).map_err(|e| peg_error(format!("invalid repo name: {s:?}"), s, e))
    }

    #[cached(
        type = "SizedCache<String, Result<Atom>>",
        create = "{ SizedCache::with_size(1000) }",
        convert = r#"{ s.to_string() }"#
    )]
    pub fn cpv(s: &str) -> Result<Atom> {
        let mut parsed_cpv =
            pkg::cpv(s).map_err(|e| peg_error(format!("invalid cpv: {s:?}"), s, e))?;
        parsed_cpv.version_str = Some(s);
        parsed_cpv.into_owned()
    }

    pub(crate) fn dep_str<'a>(s: &'a str, eapi: &'static Eapi) -> Result<ParsedAtom<'a>> {
        let (dep, mut atom) =
            pkg::dep(s, eapi).map_err(|e| peg_error(format!("invalid atom: {s:?}"), s, e))?;
        let attrs =
            pkg::cpv_or_cp(dep).map_err(|e| peg_error(format!("invalid atom: {s:?}"), dep, e))?;

        match attrs {
            (true, op, cpv, glob) => {
                let cpv_atom =
                    pkg::cpv(cpv).map_err(|e| peg_error(format!("invalid atom: {s:?}"), cpv, e))?;
                let ver = cpv_atom.version.unwrap();
                atom.category = cpv_atom.category;
                atom.package = cpv_atom.package;
                atom.version = Some(
                    ver.with_op(op, glob)
                        .map_err(|e| Error::InvalidValue(format!("invalid atom: {s:?}: {e}")))?,
                );
                atom.version_str = Some(cpv);
            }
            (false, cat, pkg, _) => {
                atom.category = cat;
                atom.package = pkg;
            }
        }

        Ok(atom)
    }

    #[cached(
        type = "SizedCache<(String, &Eapi), Result<Atom>>",
        create = "{ SizedCache::with_size(1000) }",
        convert = r#"{ (s.to_string(), eapi) }"#
    )]
    pub fn dep(s: &str, eapi: &'static Eapi) -> Result<Atom> {
        let atom = dep_str(s, eapi)?;
        atom.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexSet;

    use crate::eapi;
    use crate::macros::opt_str;
    use crate::test::TestData;

    use super::*;

    #[test]
    fn test_parse_versions() {
        // invalid deps
        let data = TestData::load().unwrap();
        let all_eapis: IndexSet<&eapi::Eapi> = eapi::EAPIS.values().cloned().collect();
        for (s, eapis) in data.invalid_atoms {
            let failing_eapis = eapi::supported(eapis).expect("failed to parse EAPI range");
            // verify parse failures
            for eapi in &failing_eapis {
                let result = parse::dep(&s, eapi);
                assert!(result.is_err(), "{s:?} didn't fail for EAPI={eapi}");
            }
            // verify parse successes
            for eapi in all_eapis.difference(&failing_eapis) {
                let result = parse::dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed for EAPI={eapi}");
            }
        }

        // convert &str to Option<Version>
        let version = |s| parse::version_with_op(s).ok();

        // good deps
        for (s, cat, pkg, ver) in [
            ("a/b", "a", "b", None),
            ("_/_", "_", "_", None),
            ("_/_-", "_", "_-", None),
            ("_.+-/_+-", "_.+-", "_+-", None),
            ("_--/_--", "_--", "_--", None),
            ("a/b-", "a", "b-", None),
            ("a/b-r100", "a", "b-r100", None),
            ("<a/b-r0-1-r2", "a", "b-r0", version("<1-r2")),
            ("<=a/b-1", "a", "b", version("<=1")),
            ("=a/b-1-r1", "a", "b", version("=1-r1")),
            ("=a/b-3*", "a", "b", version("=3*")),
            ("=a/b-3-r1*", "a", "b", version("=3-r1*")),
            ("~a/b-0", "a", "b", version("~0")),
            (">=a/b-2", "a", "b", version(">=2")),
            (">a/b-3-r0", "a", "b", version(">3-r0")),
            (">a/b-3-c-4-r3", "a", "b-3-c", version(">4-r3")),
        ] {
            for eapi in eapi::EAPIS.values() {
                let result = parse::dep(s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let atom = result.unwrap();
                assert_eq!(atom.category, cat);
                assert_eq!(atom.package, pkg);
                assert_eq!(atom.version, ver);
                assert_eq!(format!("{atom}"), s);
            }
        }
    }

    #[test]
    fn test_parse_slots() {
        // invalid deps
        for slot in ["", "+", "+0", ".a", "-b", "a@b", "0/1"] {
            let s = format!("cat/pkg:{slot}");
            assert!(parse::dep(&s, &eapi::EAPI1).is_err(), "{s:?} didn't fail");
        }

        // good deps
        for (slot_str, slot) in [
            ("0", opt_str!("0")),
            ("a", opt_str!("a")),
            ("_", opt_str!("_")),
            ("_a", opt_str!("_a")),
            ("99", opt_str!("99")),
            ("aBc", opt_str!("aBc")),
            ("a+b_c.d-e", opt_str!("a+b_c.d-e")),
        ] {
            for eapi in eapi::EAPIS.values() {
                let s = format!("cat/pkg:{slot_str}");
                let result = parse::dep(&s, eapi);
                match eapi.has(Feature::SlotDeps) {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        assert_eq!(atom.slot, slot);
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_blockers() {
        // invalid deps
        for s in ["!!!cat/pkg", "!cat/pkg-0", "!!cat/pkg-0-r1"] {
            assert!(parse::dep(s, &eapi::EAPI2).is_err(), "{s:?} didn't fail");
        }

        // non-blocker
        let atom = parse::dep("cat/pkg", &eapi::EAPI2).unwrap();
        assert!(atom.block.is_none());

        // good deps
        for (s, block) in [
            ("!cat/pkg", Some(Blocker::Weak)),
            ("!cat/pkg:0", Some(Blocker::Weak)),
            ("!!cat/pkg", Some(Blocker::Strong)),
            ("!!<cat/pkg-1", Some(Blocker::Strong)),
        ] {
            for eapi in eapi::EAPIS.values() {
                let result = parse::dep(s, eapi);
                match eapi.has(Feature::Blockers) {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(
                            result.is_ok(),
                            "{s:?} failed for EAPI {eapi}: {}",
                            result.err().unwrap()
                        );
                        let atom = result.unwrap();
                        assert_eq!(atom.block, block);
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_use_deps() {
        // invalid deps
        for use_deps in ["", "-", "-a?", "!a"] {
            let s = format!("cat/pkg[{use_deps}]");
            assert!(parse::dep(&s, &eapi::EAPI2).is_err(), "{s:?} didn't fail");
        }

        // good deps
        for use_deps in ["a", "!a?", "a,b", "-a,-b", "a?,b?", "a,b=,!c=,d?,!e?,-f"] {
            for eapi in eapi::EAPIS.values() {
                let s = format!("cat/pkg[{use_deps}]");
                let result = parse::dep(&s, eapi);
                match eapi.has(Feature::UseDeps) {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        let expected = use_deps.split(',').map(|s| s.to_string()).collect();
                        assert_eq!(atom.use_deps, Some(expected));
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_use_dep_defaults() {
        // invalid deps
        for use_dep in ["(-)", "(+)", "a()", "a(?)", "a(b)", "a(-+)", "a(++)", "a((+))", "a(-)b"] {
            let s = format!("cat/pkg[{use_dep}]");
            assert!(parse::dep(&s, &eapi::EAPI4).is_err(), "{s:?} didn't fail");
        }

        // good deps
        for use_deps in ["a(+)", "-a(-)", "a(+)?,!b(-)?", "a(-)=,!b(+)="] {
            for eapi in eapi::EAPIS.values() {
                let s = format!("cat/pkg[{use_deps}]");
                let result = parse::dep(&s, eapi);
                match eapi.has(Feature::UseDepDefaults) {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        let expected = use_deps.split(',').map(|s| s.to_string()).collect();
                        assert_eq!(atom.use_deps, Some(expected));
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_subslots() {
        // invalid deps
        for slot in ["/", "/0", "0/", "0/+1", "0//1", "0/1/2"] {
            let s = format!("cat/pkg:{slot}");
            assert!(parse::dep(&s, &eapi::EAPI5).is_err(), "{s:?} didn't fail");
        }

        // good deps
        for (slot_str, slot, subslot, slot_op) in [
            ("0/1", opt_str!("0"), opt_str!("1"), None),
            ("a/b", opt_str!("a"), opt_str!("b"), None),
            ("A/B", opt_str!("A"), opt_str!("B"), None),
            ("_/_", opt_str!("_"), opt_str!("_"), None),
            ("0/a.b+c-d_e", opt_str!("0"), opt_str!("a.b+c-d_e"), None),
        ] {
            for eapi in eapi::EAPIS.values() {
                let s = format!("cat/pkg:{slot_str}");
                let result = parse::dep(&s, eapi);
                match eapi.has(Feature::SlotOps) {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        assert_eq!(atom.slot, slot);
                        assert_eq!(atom.subslot, subslot);
                        assert_eq!(atom.slot_op, slot_op);
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_slot_ops() {
        // invalid deps
        for slot in ["*0", "=0", "*=", "=="] {
            let s = format!("cat/pkg:{slot}");
            assert!(parse::dep(&s, &eapi::EAPI5).is_err(), "{s:?} didn't fail");
        }

        // good deps
        for (slot_str, slot, subslot, slot_op) in [
            ("*", None, None, opt_str!("*")),
            ("=", None, None, opt_str!("=")),
            ("0=", opt_str!("0"), None, opt_str!("=")),
            ("a=", opt_str!("a"), None, opt_str!("=")),
            ("0/1=", opt_str!("0"), opt_str!("1"), opt_str!("=")),
            ("a/b=", opt_str!("a"), opt_str!("b"), opt_str!("=")),
        ] {
            for eapi in eapi::EAPIS.values() {
                let s = format!("cat/pkg:{slot_str}");
                let result = parse::dep(&s, eapi);
                match eapi.has(Feature::SlotOps) {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        assert_eq!(atom.slot, slot);
                        assert_eq!(atom.subslot, subslot);
                        assert_eq!(atom.slot_op, slot_op);
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_repos() {
        // invalid repos
        for s in ["", "-repo", "repo-1", "repo@path"] {
            let result = parse::repo(&s);
            assert!(result.is_err(), "{s:?} didn't fail");
        }

        // repo deps
        for repo in ["_", "a", "repo", "repo_a", "repo-a"] {
            let s = format!("cat/pkg::{repo}");

            // repo ids aren't supported in official EAPIs
            for eapi in eapi::EAPIS_OFFICIAL.values() {
                assert!(parse::dep(&s, eapi).is_err(), "{s:?} didn't fail");
            }

            let result = parse::dep(&s, &eapi::EAPI_PKGCRAFT);
            assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
            let atom = result.unwrap();
            assert_eq!(atom.repo, opt_str!(repo));
            assert_eq!(format!("{atom}"), s);
        }
    }
}
