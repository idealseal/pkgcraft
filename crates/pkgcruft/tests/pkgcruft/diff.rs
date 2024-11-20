use std::io::Write;

use pkgcraft::test::cmd;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::NamedTempFile;

use crate::replay::qa_primary_file;

#[test]
fn missing_args() {
    // missing both file args
    cmd("pkgcruft diff")
        .assert()
        .stdout("")
        .stderr(contains("OLD"))
        .failure()
        .code(2);

    // missing second file arg
    cmd("pkgcruft diff file1.json")
        .assert()
        .stdout("")
        .stderr(contains("NEW"))
        .failure()
        .code(2);
}

#[test]
fn nonexistent_files() {
    let file = qa_primary_file();

    // first
    cmd("pkgcruft diff")
        .arg("path/to/nonexistent/file1.json")
        .arg(file.path())
        .assert()
        .stdout("")
        .stderr(contains("failed loading file"))
        .failure()
        .code(2);

    // second
    cmd("pkgcruft diff")
        .arg(file.path())
        .arg("path/to/nonexistent/file1.json")
        .assert()
        .stdout("")
        .stderr(contains("failed loading file"))
        .failure()
        .code(2);

    // both
    cmd("pkgcruft diff")
        .args(["path/to/nonexistent/file1.json", "path/to/nonexistent/file2.json"])
        .assert()
        .stdout("")
        .stderr(contains("failed loading file"))
        .failure()
        .code(2);
}

#[test]
fn empty() {
    let file = qa_primary_file();
    let output = cmd("pkgcruft diff")
        .args([file.path(), file.path()])
        .output()
        .unwrap()
        .stdout;
    let output = String::from_utf8(output).unwrap();
    let output: Vec<_> = output.lines().collect();
    assert!(output.is_empty());
}

#[test]
fn output() {
    let old = indoc::indoc! {r#"
        {"kind":"UnstableOnly","scope":{"Package":"cat/pkg"},"message":"arch"}
        {"kind":"DependencyDeprecated","scope":{"Version":["cat/pkg-1-r2",null]},"message":"BDEPEND: cat/deprecated"}
        {"kind":"WhitespaceInvalid","scope":{"Version":["cat/pkg-1-r2",{"line":3,"column":28}]},"message":"character '\\u{2001}'"}
    "#};
    let new = indoc::indoc! {r#"
        {"kind":"UnstableOnly","scope":{"Package":"cat/pkg"},"message":"arch"}
        {"kind":"WhitespaceUnneeded","scope":{"Version":["cat/pkg-1-r2",{"line":3,"column":0}]},"message":"empty line"}
        {"kind":"WhitespaceInvalid","scope":{"Version":["cat/pkg-1-r2",{"line":3,"column":28}]},"message":"character '\\u{2001}'"}
    "#};

    let mut old_file = NamedTempFile::new().unwrap();
    old_file.write_all(old.as_bytes()).unwrap();
    let mut new_file = NamedTempFile::new().unwrap();
    new_file.write_all(new.as_bytes()).unwrap();

    let expected = indoc::indoc! {"
        -cat/pkg-1-r2: DependencyDeprecated: BDEPEND: cat/deprecated
        +cat/pkg-1-r2, line 3: WhitespaceUnneeded: empty line
    "};
    let expected: Vec<_> = expected.lines().collect();

    // color disabled
    let output = cmd("pkgcruft diff")
        .args([old_file.path(), new_file.path()])
        .output()
        .unwrap()
        .stdout;
    let output = String::from_utf8(output).unwrap();
    let output: Vec<_> = output.lines().collect();
    assert_eq!(&output, &expected);

    let expected = indoc::indoc! {"
        \u{1b}[31m-cat/pkg-1-r2: DependencyDeprecated: BDEPEND: cat/deprecated\u{1b}[0m
        \u{1b}[32m+cat/pkg-1-r2, line 3: WhitespaceUnneeded: empty line\u{1b}[0m
    "};
    let expected: Vec<_> = expected.lines().collect();

    // color enabled
    let output = cmd("pkgcruft diff --color true")
        .args([old_file.path(), new_file.path()])
        .output()
        .unwrap()
        .stdout;
    let output = String::from_utf8(output).unwrap();
    let output: Vec<_> = output.lines().collect();
    assert_eq!(&output, &expected);
}
