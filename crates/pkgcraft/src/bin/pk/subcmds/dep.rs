use std::process::ExitCode;

use crate::Run;

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

impl Run for Command {
    fn run(self) -> anyhow::Result<ExitCode> {
        self.command.run()
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

impl Run for Subcommand {
    fn run(self) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Compare(cmd) => cmd.run(),
            Intersect(cmd) => cmd.run(),
            Parse(cmd) => cmd.run(),
            Set(cmd) => cmd.run(),
            Sort(cmd) => cmd.run(),
        }
    }
}
