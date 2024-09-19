use std::process::ExitCode;

use anyhow::{anyhow, bail};
use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::MaybeStdinVec;
use pkgcraft::dep::Cpv;

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Comparison expressions
    #[arg(
        value_name = "EXPR",
        long_help = indoc::indoc! {r#"
            Cpv comparison expressions.

            Valid comparison expressions consist of two cpvs separated by
            whitespace with an operator between them. Supported operators
            include <, <=, ==, !=, >=, and >.

            For example, to test if one cpv is less than or equal to another
            use: `pk cpv compare "cat/pkg-1.2.3-r1 <= cat/pkg-1.2.3-r2"` which
            returns shell true (0) when run.

            Expressions are read from standard input if `-` is used."#
        }
    )]
    values: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;

        for s in self.values.iter().flatten() {
            let (lhs, op, rhs) = s
                .split_whitespace()
                .collect_tuple()
                .ok_or_else(|| anyhow!("invalid comparison format: {s}"))?;
            let lhs = Cpv::try_new(lhs)?;
            let rhs = Cpv::try_new(rhs)?;
            let result = match op {
                "<" => lhs < rhs,
                "<=" => lhs <= rhs,
                "==" => lhs == rhs,
                "!=" => lhs != rhs,
                ">=" => lhs >= rhs,
                ">" => lhs > rhs,
                _ => bail!("invalid operator: {op}"),
            };

            if !result {
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
