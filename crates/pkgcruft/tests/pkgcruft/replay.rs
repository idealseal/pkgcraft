use std::fs;
use std::io::Write;

use once_cell::sync::Lazy;
use pkgcraft::repo::Repository;
use pkgcraft::test::{cmd, TEST_DATA};
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::NamedTempFile;

/// Temporary file of all serialized reports from the primary QA test repo.
static QA_PRIMARY_FILE: Lazy<NamedTempFile> = Lazy::new(|| {
    let mut file = NamedTempFile::new().unwrap();
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    let output = cmd("pkgcruft scan -R json")
        .arg(repo.path())
        .output()
        .unwrap()
        .stdout;
    file.write_all(&output).unwrap();
    file
});

#[test]
fn nonexistent_path_target() {
    cmd("pkgcruft replay path/to/nonexistent/file.json")
        .assert()
        .stdout("")
        .stderr(contains("failed loading file"))
        .failure()
        .code(2);
}

#[test]
fn invalid_dir_target() {
    cmd("pkgcruft replay .")
        .assert()
        .stdout("")
        .stderr(contains("failed reading line: Is a directory"))
        .failure()
        .code(2);
}

#[test]
fn stdin() {
    // valid
    cmd("pkgcruft replay -")
        .write_stdin(fs::read_to_string(QA_PRIMARY_FILE.path()).unwrap())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // invalid
    cmd("pkgcruft replay -")
        .write_stdin("invalid serialized report\n")
        .assert()
        .stdout("")
        .stderr(contains("failed deserializing report"))
        .failure()
        .code(2);
}

#[test]
fn file_targets() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "invalid reports json file").unwrap();

    // invalid
    cmd("pkgcruft replay")
        .arg(file.path())
        .assert()
        .stdout("")
        .stderr(contains("failed deserializing report"))
        .failure()
        .code(2);

    // valid
    cmd("pkgcruft replay")
        .arg(QA_PRIMARY_FILE.path())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn reporters() {
    for opt in ["-R", "--reporter"] {
        for reporter in ["simple", "fancy", "json"] {
            cmd("pkgcruft replay")
                .args([opt, reporter])
                .arg(QA_PRIMARY_FILE.path())
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }
    }
}

#[test]
fn sort() {
    // serialized reports in reversed sorting order
    let reports = indoc::indoc! {r#"
        {"scope":{"Package":"x11-wm/qtile"},"kind":{"Package":"UnstableOnly"},"level":"Info","description":"x86"}
        {"scope":{"Version":"x11-wm/qtile-0.23.0-r1"},"kind":{"Version":"DeprecatedDependency"},"level":"Warning","description":"BDEPEND: media-sound/pulseaudio"}
        {"scope":{"Version":"x11-wm/qtile-0.22.1-r3"},"kind":{"Version":"DeprecatedDependency"},"level":"Warning","description":"BDEPEND: media-sound/pulseaudio"}
    "#};
    let mut expected: Vec<_> = reports.lines().collect();
    expected.reverse();

    for opt in ["-s", "--sort"] {
        let output = cmd("pkgcruft replay -R json -")
            .arg(opt)
            .write_stdin(reports)
            .output()
            .unwrap()
            .stdout;
        let data = String::from_utf8(output).unwrap();
        let data: Vec<_> = data.lines().collect();
        assert_eq!(&data, &expected);
    }
}
