use itertools::Itertools;
use pkgcraft::test::{cmd, TEST_DATA};
use predicates::prelude::*;

use crate::predicates::lines_contain;

#[test]
fn valid() {
    let intersects = TEST_DATA
        .version_toml
        .intersects
        .iter()
        .flat_map(|e| &e.vals);
    let sorting = TEST_DATA
        .version_toml
        .sorting
        .iter()
        .flat_map(|e| &e.sorted);

    cmd("pk version parse -")
        .write_stdin(intersects.chain(sorting).join("\n"))
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn invalid() {
    cmd("pk version parse 1-r2-3-r4")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure();
}

#[test]
fn format() {
    for opt in ["-f", "--format"] {
        for (args, expected) in [
            (["{OP}", ">1-r2"], ">"),
            (["{OP}", "1-r2"], "<unset>"),
            (["{VER}", "1-r2"], "1"),
            (["{REV}", "1-r2"], "2"),
            (["{REV}", "1"], "<unset>"),
        ] {
            cmd("pk version parse")
                .arg(opt)
                .args(args)
                .assert()
                .stdout(lines_contain([expected]))
                .stderr("")
                .success();
        }
    }
}
