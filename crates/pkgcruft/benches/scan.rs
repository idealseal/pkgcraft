use criterion::Criterion;

use pkgcraft::test::test_data;
use pkgcruft::scanner::Scanner;

pub fn bench(c: &mut Criterion) {
    let data = test_data();
    let (pool, repo) = data.repo("qa-primary").unwrap();

    c.bench_function("scan", |b| {
        let scanner = Scanner::new(&pool);
        let mut count = 0;
        b.iter(|| {
            count = scanner.run(repo, repo).count();
        });
        assert!(count > 0);
    });
}
