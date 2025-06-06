use std::process::ExitCode;

mod completion;
mod cpv;
mod dep;
mod pkg;
mod repo;
mod version;

#[derive(clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum Subcommand {
    /// Generate shell completion
    Completion(completion::Command),
    /// Cpv commands
    Cpv(cpv::Command),
    /// Dependency commands
    Dep(dep::Command),
    /// Package commands
    Pkg(pkg::Command),
    /// Repository commands
    Repo(repo::Command),
    /// Version commands
    Version(version::Command),
}

impl Subcommand {
    pub(super) fn run(&self, args: &crate::Command) -> anyhow::Result<ExitCode> {
        match self {
            Self::Cpv(cmd) => cmd.run(),
            Self::Dep(cmd) => cmd.run(),
            Self::Pkg(cmd) => cmd.run(args.load_config()?),
            Self::Repo(cmd) => cmd.run(args.load_config()?),
            Self::Completion(cmd) => cmd.run(),
            Self::Version(cmd) => cmd.run(),
        }
    }
}
