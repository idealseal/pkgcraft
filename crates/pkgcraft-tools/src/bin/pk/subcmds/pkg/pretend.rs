use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::{Config, Repos};
use pkgcraft::pkg::{ebuild, Pretend};
use pkgcraft::repo::set::RepoSet;
use pkgcraft::utils::bounded_jobs;
use scallop::pool::PoolIter;

use crate::args::StdinOrArgs;

use super::target_restriction;

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repository
    #[arg(short, long)]
    repo: Option<String>,

    // positionals
    /// Target packages or directories
    #[arg(value_name = "TARGET", default_value = ".")]
    targets: Vec<String>,
}

// TODO: use configured ebuild repos instead of raw ones
// TODO: support binpkg repos
impl Command {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine target repo set
        let repos = if let Some(repo) = self.repo.as_ref() {
            let repo = if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
            } else if Path::new(repo).exists() {
                config.add_nested_repo_path(repo, 0, repo, true)
            } else {
                anyhow::bail!("unknown repo: {repo}")
            }?;
            RepoSet::from_iter([&repo])
        } else {
            config.repos.set(Repos::Ebuild)
        };

        let func = |pkg: ebuild::raw::Pkg| -> scallop::Result<Option<String>> {
            let pkg = ebuild::Pkg::try_from(pkg)?;
            pkg.pretend()
        };

        // loop over targets, tracking overall failure status
        let jobs = bounded_jobs(self.jobs.unwrap_or_default());
        let mut status = ExitCode::SUCCESS;
        for target in self.targets.stdin_or_args().split_whitespace() {
            // determine target restriction
            let (repos, restrict) = target_restriction(config, &repos, &target)?;

            // find matching packages from targeted repos
            let pkgs = repos.ebuild().flat_map(|r| r.iter_raw_restrict(&restrict));

            // run pkg_pretend across selected pkgs
            let mut handle = io::stderr().lock();
            for result in PoolIter::new(jobs, pkgs, func, true)? {
                match result {
                    Err(e) => {
                        status = ExitCode::FAILURE;
                        writeln!(handle, "{e}")?;
                    }
                    Ok(Some(s)) => writeln!(handle, "{s}")?,
                    Ok(None) => (),
                }
            }
        }

        Ok(status)
    }
}
