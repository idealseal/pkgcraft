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
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(Reporter::VARIANTS)
            .map(|s| s.parse::<Reporter>().unwrap()),
    )]
    reporter: Option<Reporter>,

    /// Format string for the format reporter
    #[arg(long, required_if_eq("reporter", "format"))]
    format: Option<String>,
}

impl ReporterOptions {
    pub(crate) fn collapse(&self) -> Reporter {
        let mut reporter = self.reporter.clone().unwrap_or_default();

        if let Reporter::Format(r) = &mut reporter {
            r.format = self.format.clone().unwrap_or_default();
        }

        reporter
    }
}
