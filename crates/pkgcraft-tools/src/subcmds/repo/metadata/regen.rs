use std::io::{stdout, IsTerminal};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::{Cache, CacheFormat};
use pkgcraft::utils::bounded_thread_pool;

#[derive(Args)]
#[clap(next_help_heading = "Regen options")]
pub(crate) struct Command {
    /// Parallel jobs to run
    #[arg(short, long, default_value_t = num_cpus::get())]
    jobs: usize,

    /// Force regeneration to occur
    #[arg(short, long)]
    force: bool,

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

    /// Update local USE cache
    #[arg(long)]
    use_local: bool,

    // positionals
    /// Target repository
    #[arg(default_value = ".", help_heading = "Arguments")]
    repo: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repo = Targets::new(config)
            .finalize_repos([&self.repo])?
            .ebuild_repo()?;

        let format = self.format.unwrap_or(repo.metadata().cache().format());
        let cache = if let Some(path) = self.path.as_ref() {
            format.from_path(path)
        } else {
            format.from_repo(&repo)
        };

        // build custom, global thread pool when limiting jobs
        bounded_thread_pool(self.jobs);

        cache
            .regen(&repo)
            .force(self.force)
            .progress(stdout().is_terminal() && !self.no_progress)
            .output(self.output)
            .run()?;

        if self.use_local {
            repo.metadata().use_local_update(&repo)?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
