use scallop::variables::{ScopedVariable, ShellVariable, Variable};
use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;
use crate::traits::SourceBash;

use super::make_builtin;

const LONG_DOC: &str = "Sources the given list of eclasses.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    let eclasses = build.ebuild_repo()?.eclasses();

    // force incrementals to be restored between nested inherits
    let incrementals: Vec<(_, _)> = build
        .eapi()
        .incremental_keys()
        .iter()
        .map(|k| (*k, ScopedVariable::new(k)))
        .collect();

    let mut eclass_var = ScopedVariable::new("ECLASS");
    let mut inherited_var = Variable::new("INHERITED");

    for name in args {
        let eclass = eclasses
            .get(*name)
            .ok_or_else(|| Error::Base(format!("unknown eclass: {name}")))?;

        // track direct inherits
        if !build.scope.is_eclass() {
            build.inherit.insert(eclass);
        }

        // track all inherits
        if !build.inherited.insert(eclass) {
            // skip previous and nested inherits
            continue;
        }

        // track build scope
        let _scope = build.scoped(eclass);

        // update $ECLASS and $INHERITED variables
        eclass_var.bind(name, None, None)?;
        inherited_var.append(format!(" {name}"))?;

        eclass.source_bash().map_err(|e| {
            // strip path prefix from bash error
            let s = e.to_string();
            let s = if s.starts_with('/') {
                match s.split_once(": ") {
                    Some((_, suffix)) => suffix,
                    None => s.as_str(),
                }
            } else {
                s.as_str()
            };
            Error::Base(format!("failed loading eclass: {name}: {s}"))
        })?;

        // append metadata keys that incrementally accumulate
        for (key, var) in &incrementals {
            if let Some(data) = var.to_vec() {
                build.incrementals.entry(*key).or_default().extend(data);
            }
        }
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "inherit eclass1 eclass2";
make_builtin!("inherit", inherit_builtin);

#[cfg(test)]
mod tests {
    use scallop::variables::{optional, string_vec};

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::Source;
    use crate::shell::BuildData;
    use crate::test::{assert_ordered_eq, TEST_DATA};

    use super::super::{assert_invalid_args, cmd_scope_tests, inherit};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(inherit, &[0]);
    }

    #[test]
    fn nonexistent() {
        let mut config = Config::default();
        let t = config.temp_repo("test1", 0, None).unwrap();

        // single
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        let r = inherit(&["nonexistent"]);
        assert_err_re!(r, r"^unknown eclass: nonexistent");

        // multiple
        let r = inherit(&["e1", "e2"]);
        assert_err_re!(r, r"^unknown eclass: e1");

        // multiple with existing and nonexistent
        let t = config.temp_repo("test2", 0, None).unwrap();
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        let eclass = indoc::indoc! {r#"
            # stub eclass
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let r = inherit(&["e1", "e2"]);
        assert_err_re!(r, r"^unknown eclass: e2");
    }

    #[test]
    fn source_failure() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            unknown_cmd
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        let r = inherit(&["e1"]);
        assert_err_re!(r, "^failed loading eclass: e1: line 2: unknown command: unknown_cmd$");
    }

    #[test]
    fn single() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        let build = get_build_mut();
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e1"]).unwrap();
        assert_ordered_eq!(build.inherit.iter().map(|e| e.name()), ["e1"]);
        assert_ordered_eq!(build.inherited.iter().map(|e| e.name()), ["e1"]);
        assert_eq!(string_vec("INHERITED").unwrap(), ["e1"]);
    }

    #[test]
    fn multiple() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
            inherit e2
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e2 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e2", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        let build = get_build_mut();
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e1"]).unwrap();
        assert_ordered_eq!(build.inherit.iter().map(|e| e.name()), ["e1"]);
        assert_ordered_eq!(build.inherited.iter().map(|e| e.name()), ["e1", "e2"]);
        assert_eq!(string_vec("INHERITED").unwrap(), ["e1", "e2"]);
    }

    #[test]
    fn nested_single() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
            [[ ${ECLASS} == e2 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e2", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        let build = get_build_mut();
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e2"]).unwrap();
        assert_ordered_eq!(build.inherit.iter().map(|e| e.name()), ["e2"]);
        assert_ordered_eq!(build.inherited.iter().map(|e| e.name()), ["e2", "e1"]);
        assert_eq!(string_vec("INHERITED").unwrap(), ["e2", "e1"]);
    }

    #[test]
    fn nested_multiple() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
            [[ ${ECLASS} == e2 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e2", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e2
            [[ ${ECLASS} == e3 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e3", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        let build = get_build_mut();
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e3"]).unwrap();
        assert_ordered_eq!(build.inherit.iter().map(|e| e.name()), ["e3"]);
        assert_ordered_eq!(build.inherited.iter().map(|e| e.name()), ["e3", "e2", "e1"]);
        assert_eq!(string_vec("INHERITED").unwrap(), ["e3", "e2", "e1"]);
    }

    #[test]
    fn nested_errors() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            die "${ECLASS} failed"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
        "#};
        t.create_eclass("e2", eclass).unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e2
            DESCRIPTION="testing for nested eclass errors"
            SLOT=0
        "#};
        let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", data).unwrap();
        let r = raw_pkg.source();
        assert_err_re!(
            r,
            r"^line 2: inherit: error: failed loading eclass: e2: line 2: inherit: error: failed loading eclass: e1: line 1: die: error: e1 failed$"
        );
    }

    #[test]
    fn pkg_env() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing for eclass env transit"
            SLOT=0
        "#};
        let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", data).unwrap();
        raw_pkg.source().unwrap();
        assert!(optional("ECLASS").is_none(), "$ECLASS shouldn't be defined");
        assert_eq!(string_vec("INHERITED").unwrap(), ["e1"]);
    }

    #[test]
    fn cyclic() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e2
            VAR+="e0"
        "#};
        t.create_eclass("e0", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e0
            VAR+="e1"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
            VAR+="e2"
        "#};
        t.create_eclass("e2", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit r
            VAR+="r"
        "#};
        t.create_eclass("r", eclass).unwrap();

        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        let build = get_build_mut();
        let mut var = Variable::new("VAR");

        // verify previous inherits are skipped
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["e1", "e2"]).unwrap();
        assert_ordered_eq!(build.inherit.iter().map(|e| e.name()), ["e1", "e2"]);
        assert_ordered_eq!(build.inherited.iter().map(|e| e.name()), ["e1", "e0", "e2"]);
        assert_eq!(var.optional().unwrap(), "e2e0e1");

        // verify nested inherits are skipped
        BuildData::from_raw_pkg(&raw_pkg);
        var.unbind().unwrap();
        inherit(&["e2", "e1"]).unwrap();
        assert_ordered_eq!(build.inherit.iter().map(|e| e.name()), ["e2", "e1"]);
        assert_ordered_eq!(build.inherited.iter().map(|e| e.name()), ["e2", "e1", "e0"]);
        assert_eq!(var.optional().unwrap(), "e0e1e2");

        // verify recursive inherits are skipped
        BuildData::from_raw_pkg(&raw_pkg);
        var.unbind().unwrap();
        inherit(&["r"]).unwrap();
        assert_ordered_eq!(build.inherit.iter().map(|e| e.name()), ["r"]);
        assert_ordered_eq!(build.inherited.iter().map(|e| e.name()), ["r"]);
        assert_eq!(var.optional().unwrap(), "r");
    }

    #[test]
    fn overlay() {
        let raw_pkg = TEST_DATA.ebuild_raw_pkg("=cat/pkg-1::secondary").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        inherit(&["b", "c"]).unwrap();
        let build = get_build_mut();
        assert_ordered_eq!(build.inherit.iter().map(|e| e.name()), ["b", "c"]);
        assert_ordered_eq!(build.inherited.iter().map(|e| e.name()), ["b", "a", "c"]);
        assert_eq!(string_vec("INHERITED").unwrap(), ["b", "a", "c"]);
    }
}
