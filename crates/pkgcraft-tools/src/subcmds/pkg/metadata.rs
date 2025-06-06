use std::io::{IsTerminal, stdout};
use std::process::ExitCode;

use clap::{Args, builder::ArgPredicate};
use pkgcraft::cli::{MaybeStdinVec, Targets};
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::{Cache, CacheFormat};
use pkgcraft::repo::{PkgRepository, RepoFormat};
use pkgcraft::utils::bounded_thread_pool;

#[derive(Args)]
#[clap(next_help_heading = "Metadata options")]
pub(crate) struct Command {
    /// Parallel jobs to run
    #[arg(short, long, default_value_t = num_cpus::get())]
    jobs: usize,

    /// Force regeneration to occur
    #[arg(short, long)]
    force: bool,

    /// Verify metadata without updating cache
    #[arg(short = 'V', long)]
    verify: bool,

    /// Remove cache entries
    #[arg(short = 'R', long)]
    remove: bool,

    /// Custom cache path
    #[arg(short, long)]
    path: Option<String>,

    /// Disable progress bar
    #[arg(short, long)]
    no_progress: bool,

    /// Capture stderr and stdout
    #[arg(short, long)]
    output: bool,

    /// Custom cache format
    #[arg(long)]
    format: Option<CacheFormat>,

    /// Target repo
    #[arg(short, long)]
    repo: Option<String>,

    // positionals
    /// Target packages or paths
    #[arg(
        value_name = "TARGET",
        // default to the current working directory
        default_value = ".",
        // default to all packages when targeting a repo
        default_value_if("repo", ArgPredicate::IsPresent, Some("*")),
        help_heading = "Arguments",
    )]
    targets: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // build custom, global thread pool when limiting jobs
        bounded_thread_pool(self.jobs);

        // convert targets to restrictions
        let targets = Targets::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .pkg_targets(self.targets.iter().flatten())?
            .collapse();

        for (repo_set, restrict) in targets {
            for repo in repo_set.iter_ebuild() {
                let format = self.format.unwrap_or(repo.metadata().cache().format());

                let cache = if let Some(path) = self.path.as_ref() {
                    format.from_path(path)
                } else {
                    format.from_repo(repo)
                };

                if self.remove {
                    for cpv in repo.iter_cpv_restrict(&restrict) {
                        cache.remove_entry(&cpv)?;
                    }
                } else {
                    cache
                        .regen(repo)
                        .force(self.force)
                        .progress(stdout().is_terminal() && !self.no_progress)
                        .output(self.output)
                        .verify(self.verify)
                        .targets(restrict.clone())
                        .run()?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
