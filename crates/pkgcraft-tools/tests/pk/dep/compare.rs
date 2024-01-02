use itertools::Itertools;
use pkgcraft::test::{cmd, TEST_DATA};

use crate::predicates::lines_contain;

#[test]
fn stdin() {
    let exprs = TEST_DATA.dep_toml.compares().map(|(s, _)| s).join("\n");
    cmd("pk dep compare -")
        .write_stdin(exprs)
        .assert()
        .success();

    let exprs = TEST_DATA
        .version_toml
        .compares()
        .map(|(_, (s1, op, s2))| format!("=cat/pkg-{s1} {op} =cat/pkg-{s2}"))
        .join("\n");
    cmd("pk dep compare -")
        .write_stdin(exprs)
        .assert()
        .success();
}

#[test]
fn args() {
    // invalid expression
    cmd("pk dep compare")
        .arg("cat/pkg<cat/pkg")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid comparison format: cat/pkg<cat/pkg"]))
        .failure()
        .code(2);

    // invalid operator
    for op in ["~=", "=", "+="] {
        cmd("pk dep compare")
            .arg(format!("cat/pkg {op} cat/pkg"))
            .assert()
            .stdout("")
            .stderr(lines_contain([format!("invalid operator: {op}")]))
            .failure()
            .code(2);
    }

    // invalid dep
    cmd("pk dep compare")
        .arg("=cat/pkg-1 >= cat/pkg-1")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid dep: cat/pkg-1"]))
        .failure()
        .code(2);

    // false expression
    cmd("pk dep compare")
        .arg("cat/pkg > cat/pkg")
        .assert()
        .failure()
        .code(1);

    // true expression
    cmd("pk dep compare")
        .arg("cat/pkg == cat/pkg")
        .assert()
        .success();
}
