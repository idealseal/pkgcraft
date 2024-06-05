use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{bounded, Receiver, Sender};
use indexmap::IndexSet;
use pkgcraft::repo::{ebuild, Repo};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::bounded_jobs;
use strum::IntoEnumIterator;

use crate::check::Check;
use crate::report::{Report, ReportKind};
use crate::runner::SyncCheckRunner;

#[derive(Debug)]
pub struct Scanner {
    jobs: usize,
    checks: IndexSet<Check>,
    reports: IndexSet<ReportKind>,
    exit: IndexSet<ReportKind>,
    failed: Arc<AtomicBool>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            jobs: bounded_jobs(0),
            checks: Check::iter_default().collect(),
            reports: ReportKind::iter().collect(),
            exit: Default::default(),
            failed: Arc::new(Default::default()),
        }
    }
}

impl Scanner {
    /// Create a new scanner with all checks enabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of parallel scanner jobs to run.
    pub fn jobs(mut self, jobs: usize) -> Self {
        self.jobs = bounded_jobs(jobs);
        self
    }

    /// Set the checks to run.
    pub fn checks<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = Check>,
    {
        self.checks = values.into_iter().map(Into::into).collect();
        self
    }

    /// Set enabled report variants.
    pub fn reports<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.reports = values.into_iter().collect();
        self
    }

    /// Set report variants that trigger exit code failures.
    pub fn exit<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.exit = values.into_iter().collect();
        self
    }

    /// Return true if the scanning process failed, false otherwise.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    /// Run the scanner returning an iterator of reports.
    pub fn run<I, R>(&self, repo: &Repo, restricts: I) -> impl Iterator<Item = Report>
    where
        I: IntoIterator<Item = R>,
        R: Into<Restrict>,
    {
        let restricts = restricts.into_iter().map(Into::into).collect();
        let (restrict_tx, restrict_rx) = bounded(self.jobs);
        let (reports_tx, reports_rx) = bounded(self.jobs);
        let filter = Arc::new(self.reports.clone());
        let exit = Arc::new(self.exit.clone());

        match repo {
            Repo::Ebuild(r) => {
                let runner = Arc::new(SyncCheckRunner::new(r, &self.checks));
                Iter {
                    reports_rx,
                    _producer: producer(r.clone(), restricts, restrict_tx),
                    _workers: (0..self.jobs)
                        .map(|_| {
                            worker(
                                runner.clone(),
                                filter.clone(),
                                exit.clone(),
                                self.failed.clone(),
                                restrict_rx.clone(),
                                reports_tx.clone(),
                            )
                        })
                        .collect(),
                    reports: Default::default(),
                }
            }
            _ => todo!("add support for other repo types"),
        }
    }
}

// TODO: use multiple producers to push restrictions
/// Create a producer thread that sends restrictions over the channel to the workers.
fn producer(
    repo: Arc<ebuild::Repo>,
    restricts: Vec<Restrict>,
    tx: Sender<Restrict>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for r in restricts {
            for cpn in repo.iter_cpn_restrict(r) {
                tx.send(cpn.into()).ok();
            }
        }
    })
}

pub(crate) struct ReportFilter {
    reports: Option<Vec<Report>>,
    filter: Arc<IndexSet<ReportKind>>,
    exit: Arc<IndexSet<ReportKind>>,
    failed: Arc<AtomicBool>,
    tx: Sender<Vec<Report>>,
}

impl ReportFilter {
    /// Conditionally add a report based on filter inclusion.
    pub(crate) fn report(&mut self, report: Report) {
        if self.filter.contains(report.kind()) {
            if self.exit.contains(report.kind()) {
                self.failed.store(true, Ordering::Relaxed);
            }

            if let Some(x) = self.reports.as_mut() {
                x.push(report);
            }
        }
    }

    /// Sort existing reports and send them to the iterator.
    fn process(&mut self) {
        if let Some(mut reports) = self.reports.take() {
            self.reports = Some(Default::default());
            reports.sort();
            self.tx.send(reports).ok();
        }
    }
}

/// Create worker thread that receives restrictions and send reports over the channel.
fn worker(
    runner: Arc<SyncCheckRunner>,
    filter: Arc<IndexSet<ReportKind>>,
    exit: Arc<IndexSet<ReportKind>>,
    failed: Arc<AtomicBool>,
    rx: Receiver<Restrict>,
    tx: Sender<Vec<Report>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut filter = ReportFilter {
            reports: Some(Default::default()),
            filter,
            exit,
            failed,
            tx,
        };

        for restrict in rx {
            runner.run(&restrict, &mut filter);
            filter.process();
        }
    })
}

struct Iter {
    reports_rx: Receiver<Vec<Report>>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
    reports: VecDeque<Report>,
}

impl Iterator for Iter {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        self.reports.pop_front().or_else(|| {
            self.reports_rx.recv().ok().and_then(|reports| {
                self.reports.extend(reports);
                self.next()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::dep::Dep;
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;
    use pretty_assertions::assert_eq;

    use crate::check::CheckKind;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn run() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let repo_path = repo.path();

        // repo level
        let scanner = Scanner::new().jobs(1);
        let expected = glob_reports!("{repo_path}/**/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // specific checks
        let check = CheckKind::Dependency.into();
        let scanner = Scanner::new().jobs(1).checks([check]);
        let expected = glob_reports!("{repo_path}/Dependency/**/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // specific reports
        let scanner = Scanner::new()
            .jobs(1)
            .reports([ReportKind::DependencyDeprecated]);
        let expected = glob_reports!("{repo_path}/Dependency/DependencyDeprecated/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // no checks
        let scanner = Scanner::new().jobs(1).checks([]);
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);

        // no reports
        let scanner = Scanner::new().jobs(1).reports([]);
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);

        // non-matching restriction
        let scanner = Scanner::new().jobs(1);
        let dep = Dep::try_new("nonexistent/pkg").unwrap();
        let reports: Vec<_> = scanner.run(repo, [&dep]).collect();
        assert_eq!(&reports, &[]);

        // empty repo
        let repo = TEST_DATA.repo("empty").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }

    #[test]
    fn failed() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();

        // no reports flagged for failures
        let scanner = Scanner::new().jobs(1);
        scanner.run(repo, [repo]).count();
        assert!(!scanner.failed());

        // fail on specified report variant
        let scanner = Scanner::new()
            .jobs(1)
            .exit([ReportKind::DependencyDeprecated]);
        scanner.run(repo, [repo]).count();
        assert!(scanner.failed());
    }
}
