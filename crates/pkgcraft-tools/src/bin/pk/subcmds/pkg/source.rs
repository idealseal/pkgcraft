use std::io::stdin;
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;
use std::time::{Duration, Instant};

use clap::Args;
use is_terminal::IsTerminal;
use itertools::Either;
use pkgcraft::config::{Config, Repos};
use pkgcraft::pkg::ebuild::RawPkg;
use pkgcraft::pkg::SourceablePackage;
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::RepoFormat::Ebuild as EbuildRepo;
use scallop::pool::PoolIter;
use tracing::error;

use crate::args::bounded_jobs;

use super::target_restriction;

/// Duration bound to apply against elapsed time values.
#[derive(Debug, Copy, Clone)]
enum Bound {
    Less(Duration),
    LessOrEqual(Duration),
    Greater(Duration),
    GreaterOrEqual(Duration),
}

impl Bound {
    fn matches(&self, duration: &Duration) -> bool {
        match self {
            Self::Less(bound) => duration < bound,
            Self::LessOrEqual(bound) => duration <= bound,
            Self::GreaterOrEqual(bound) => duration >= bound,
            Self::Greater(bound) => duration > bound,
        }
    }
}

impl FromStr for Bound {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let (bound, val): (fn(Duration) -> Self, &str) = {
            // TODO: use an actual parser
            if let Some(v) = s.strip_prefix(">=") {
                (Self::GreaterOrEqual, v)
            } else if let Some(v) = s.strip_prefix('>') {
                (Self::Greater, v)
            } else if let Some(v) = s.strip_prefix("<=") {
                (Self::LessOrEqual, v)
            } else if let Some(v) = s.strip_prefix('<') {
                (Self::Less, v)
            } else {
                (Self::GreaterOrEqual, s)
            }
        };

        let val = humantime::Duration::from_str(val)?;
        Ok(bound(val.into()))
    }
}

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repository
    #[arg(short, long)]
    repo: Option<String>,

    /// Benchmark sourcing for a given duration per package
    #[arg(long)]
    bench: Option<humantime::Duration>,

    /// Bounds applied to elapsed time
    #[arg(short, long)]
    bound: Vec<Bound>,

    // positionals
    /// Target packages or directories
    #[arg(value_name = "TARGET", default_value = ".")]
    targets: Vec<String>,
}

// Truncate a duration to microsecond precision.
macro_rules! micros {
    ($val:expr) => {{
        let val = $val.as_micros().try_into().expect("duration overflow");
        Duration::from_micros(val)
    }};
}

/// Run package sourcing benchmarks for a given amount of seconds per package.
fn benchmark<'a, I>(duration: Duration, jobs: usize, pkgs: I) -> anyhow::Result<bool>
where
    I: Iterator<Item = RawPkg<'a>>,
{
    let mut failed = false;
    let func = |pkg: RawPkg| -> scallop::Result<(String, Vec<Duration>)> {
        let mut data = vec![];
        let mut elapsed = Duration::new(0, 0);
        while elapsed < duration {
            let start = Instant::now();
            pkg.source()?;
            let source_elapsed = micros!(start.elapsed());
            data.push(source_elapsed);
            elapsed += source_elapsed;
            scallop::shell::reset(&[]);
        }
        Ok((pkg.to_string(), data))
    };

    for r in PoolIter::new(jobs, pkgs, func, true)? {
        match r {
            Ok((pkg, data)) => {
                let n = data.len() as u64;
                let micros: Vec<u64> = data
                    .iter()
                    .map(|v| v.as_micros().try_into().unwrap())
                    .collect();
                let min = Duration::from_micros(*micros.iter().min().unwrap());
                let max = Duration::from_micros(*micros.iter().max().unwrap());
                let total: u64 = micros.iter().sum();
                let mean: u64 = total / n;
                let variance = (micros
                    .iter()
                    .map(|v| (*v as i64 - mean as i64).pow(2))
                    .sum::<i64>()) as f64
                    / n as f64;
                let std_dev = Duration::from_micros(variance.sqrt().round() as u64);
                let mean = Duration::from_micros(mean);
                println!(
                    "{pkg}: min: {min:?}, mean: {mean:?}, max: {max:?}, σ = {std_dev:?}, N = {n}"
                )
            }
            Err(e) => {
                failed = true;
                error!("{e}");
            }
        }
    }

    Ok(failed)
}

/// Run package sourcing a single time per package.
fn source<'a, I>(jobs: usize, pkgs: I, bound: &[Bound]) -> anyhow::Result<bool>
where
    I: Iterator<Item = RawPkg<'a>>,
{
    let mut failed = false;
    let func = |pkg: RawPkg| -> scallop::Result<(String, Duration)> {
        let start = Instant::now();
        pkg.source()?;
        let elapsed = micros!(start.elapsed());
        Ok((pkg.to_string(), elapsed))
    };

    for r in PoolIter::new(jobs, pkgs, func, true)? {
        match r {
            Ok((pkg, elapsed)) => {
                if bound.iter().all(|b| b.matches(&elapsed)) {
                    println!("{pkg}: {elapsed:?}")
                }
            }
            Err(e) => {
                failed = true;
                error!("{e}");
            }
        }
    }

    Ok(failed)
}

impl Command {
    pub(super) fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        // determine target repo set
        let repos = if let Some(repo) = self.repo.as_ref() {
            let repo = if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
            } else if Path::new(repo).exists() {
                EbuildRepo.load_from_path(repo, 0, repo, true)
            } else {
                anyhow::bail!("unknown repo: {repo}")
            }?;
            RepoSet::new([&repo])
        } else {
            config.repos.set(Repos::Ebuild)
        };

        // pull targets from args or stdin
        let targets = if stdin().is_terminal() {
            Either::Left(self.targets.into_iter())
        } else {
            Either::Right(stdin().lines().map_while(Result::ok))
        };

        // loop over targets, tracking overall failure status
        let jobs = bounded_jobs(self.jobs)?;
        let mut failed = false;
        for target in targets {
            // determine target restriction
            let (repos, restrict) = target_restriction(&repos, &target)?;

            // find matching packages from targeted repos
            let pkgs = repos.ebuild().flat_map(|r| r.iter_raw_restrict(&restrict));

            let target_failed = if let Some(duration) = self.bench {
                benchmark(duration.into(), jobs, pkgs)
            } else {
                source(jobs, pkgs, &self.bound)
            }?;

            if target_failed {
                failed = true;
            }
        }

        if failed {
            Ok(ExitCode::FAILURE)
        } else {
            Ok(ExitCode::SUCCESS)
        }
    }
}
