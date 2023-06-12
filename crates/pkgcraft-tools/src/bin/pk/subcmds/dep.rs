use std::process::ExitCode;

use pkgcraft::config::Config;

mod compare;
mod intersect;
mod parse;
mod set;
mod sort;

#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Command {
    pub(super) fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        self.command.run(config)
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Compare two deps
    Compare(compare::Command),
    /// Determine if two deps intersect
    Intersect(intersect::Command),
    /// Parse a dep and optionally print formatted output
    Parse(parse::Command),
    /// Collapse input into a set of deps
    Set(set::Command),
    /// Sort deps
    Sort(sort::Command),
}

impl Subcommand {
    fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Compare(cmd) => cmd.run(config),
            Intersect(cmd) => cmd.run(config),
            Parse(cmd) => cmd.run(config),
            Set(cmd) => cmd.run(config),
            Sort(cmd) => cmd.run(config),
        }
    }
}
