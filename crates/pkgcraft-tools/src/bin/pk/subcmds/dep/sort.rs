use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Dep;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub struct Command {
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let deps: Result<Vec<Dep>, _> = self
            .vals
            .stdin_or_args()
            .split_whitespace()
            .map(|s| s.parse())
            .collect();

        let mut deps = deps?;
        deps.sort();

        let mut handle = io::stdout().lock();
        for d in deps {
            writeln!(handle, "{d}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
