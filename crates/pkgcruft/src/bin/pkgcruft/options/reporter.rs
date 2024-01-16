use std::str::FromStr;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use pkgcruft::reporter::Reporter;
use strum::VariantNames;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Reporter options"))]
pub(crate) struct ReporterOptions {
    /// Reporter to use
    #[arg(
        short = 'R',
        long,
        default_value = "fancy",
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(Reporter::VARIANTS)
            .map(|s| Reporter::from_str(&s).unwrap()),
    )]
    reporter: Reporter,

    /// Format string for the format reporter
    #[arg(short, long)]
    format: Option<String>,
}

impl ReporterOptions {
    pub(crate) fn collapse(mut self) -> anyhow::Result<Reporter> {
        self.reporter.format(self.format.unwrap_or_default())?;
        Ok(self.reporter)
    }
}