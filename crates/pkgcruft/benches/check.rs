use std::env;

use criterion::Criterion;
use pkgcraft::repo::Repo;
use pkgcruft::check::CheckKind;
use pkgcruft::scanner::Scanner;
use strum::IntoEnumIterator;

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("Check");
    group.sample_size(10);

    if let Ok(path) = env::var("PKGCRUFT_BENCH_REPO") {
        let repo = Repo::from_path(&path, &path, 0, true).unwrap();
        // TODO: checkout a specific commit

        // run benchmark for every check
        for check in CheckKind::iter() {
            group.bench_function(check.to_string(), |b| {
                let scanner = Scanner::new().checks([check]);
                b.iter(|| scanner.run(&repo, [&repo]).count());
            });
        }
    } else {
        eprintln!("skipping check benchmarks: PKGCRUFT_BENCH_REPO unset");
    }
}
