use std::collections::HashMap;
use std::io::Write;

use colored::{Color, Colorize};
use itertools::Itertools;
use strfmt::strfmt;
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

use crate::report::{Report, ReportScope};
use crate::Error;

#[derive(AsRefStr, Display, EnumIter, EnumString, VariantNames, Debug, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum Reporter {
    Simple(SimpleReporter),
    Fancy(FancyReporter),
    Json(JsonReporter),
    Format(FormatReporter),
}

impl Default for Reporter {
    fn default() -> Self {
        Reporter::Fancy(Default::default())
    }
}

impl Reporter {
    /// Run a report through a reporter.
    pub fn report(&mut self, report: &Report, output: &mut dyn Write) -> crate::Result<()> {
        match self {
            Self::Simple(r) => r.report(report, output),
            Self::Fancy(r) => r.report(report, output),
            Self::Json(r) => r.report(report, output),
            Self::Format(r) => r.report(report, output),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SimpleReporter;

impl From<SimpleReporter> for Reporter {
    fn from(value: SimpleReporter) -> Self {
        Self::Simple(value)
    }
}

impl SimpleReporter {
    fn report(&mut self, report: &Report, output: &mut dyn Write) -> crate::Result<()> {
        writeln!(output, "{report}")?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct FancyReporter {
    prev_key: Option<String>,
}

impl From<FancyReporter> for Reporter {
    fn from(value: FancyReporter) -> Self {
        Self::Fancy(value)
    }
}

impl FancyReporter {
    fn report(&mut self, report: &Report, output: &mut dyn Write) -> crate::Result<()> {
        let key = match report.scope() {
            ReportScope::Version(cpv, _) => cpv.cpn().to_string(),
            ReportScope::Package(cpn) => cpn.to_string(),
            ReportScope::Category(cat) => cat.to_string(),
            ReportScope::Repo(repo) => repo.to_string(),
        };

        if !self
            .prev_key
            .as_ref()
            .map(|prev| prev == &key)
            .unwrap_or_default()
        {
            if self.prev_key.is_some() {
                writeln!(output)?;
            }
            writeln!(output, "{}", key.color(Color::Blue).bold())?;
            self.prev_key = Some(key);
        }

        write!(output, "  {}: ", report.kind().as_ref().color(report.level()))?;

        if let ReportScope::Version(cpv, line) = report.scope() {
            let line = line.map(|x| format!(", line {x}")).unwrap_or_default();
            write!(output, "version {}{line}: ", cpv.version())?;
        }
        writeln!(output, "{}", report.message())?;

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct JsonReporter;

impl From<JsonReporter> for Reporter {
    fn from(value: JsonReporter) -> Self {
        Self::Json(value)
    }
}

impl JsonReporter {
    fn report(&self, report: &Report, output: &mut dyn Write) -> crate::Result<()> {
        writeln!(output, "{}", report.to_json())?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct FormatReporter {
    pub format: String,
}

impl From<FormatReporter> for Reporter {
    fn from(value: FormatReporter) -> Self {
        Self::Format(value)
    }
}

impl FormatReporter {
    fn report(&self, report: &Report, output: &mut dyn Write) -> crate::Result<()> {
        let mut attrs: HashMap<_, _> = [("name".to_string(), report.kind().to_string())]
            .into_iter()
            .collect();

        match report.scope() {
            ReportScope::Version(cpv, _) => {
                let category = cpv.category().to_string();
                let package = cpv.package().to_string();
                let version = cpv.version().to_string();
                let ebuild = format!("{package}-{version}.ebuild");
                attrs.extend([
                    ("path".to_string(), format!("{category}/{package}/{ebuild}")),
                    ("ebuild".to_string(), ebuild),
                    ("category".to_string(), category),
                    ("package".to_string(), package),
                    ("version".to_string(), version),
                    ("cpv".to_string(), cpv.to_string()),
                ]);
            }
            ReportScope::Package(cpn) => {
                attrs.extend([
                    ("category".to_string(), cpn.category().to_string()),
                    ("package".to_string(), cpn.package().to_string()),
                    ("cpn".to_string(), cpn.to_string()),
                ]);
            }
            ReportScope::Category(cat) => {
                attrs.extend([("category".to_string(), cat.to_string())]);
            }
            ReportScope::Repo(repo) => {
                attrs.extend([("repo".to_string(), repo.to_string())]);
            }
        }

        let s = strfmt(&self.format, &attrs).map_err(|e| {
            let supported = attrs.keys().sorted().join(", ");
            Error::InvalidValue(format!(
                "{}: invalid output format: {e}\n  [possible attributes: {supported}]",
                report.kind()
            ))
        })?;
        if !s.is_empty() {
            writeln!(output, "{s}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    static REPORTS: &str = indoc::indoc! {r#"
        {"kind":"UnstableOnly","scope":{"Package":"cat/pkg"},"message":"arch"}
        {"kind":"DependencyDeprecated","scope":{"Version":["cat/pkg-1-r2",null]},"message":"BDEPEND: cat/deprecated"}
    "#};

    fn report<R: Into<Reporter>>(reporter: R) -> String {
        let mut reporter = reporter.into();
        let reports = REPORTS.lines().map(|x| Report::from_json(x).unwrap());
        let mut output = Vec::new();

        for report in reports {
            reporter.report(&report, &mut output).unwrap();
        }

        String::from_utf8(output).unwrap()
    }

    #[test]
    fn simple() {
        let expected = indoc::indoc! {"
            cat/pkg: UnstableOnly: arch
            cat/pkg-1-r2: DependencyDeprecated: BDEPEND: cat/deprecated
        "};

        let output = report(SimpleReporter);
        assert_eq!(expected, &output);
    }

    #[test]
    fn fancy() {
        let expected = indoc::indoc! {"
            cat/pkg
              UnstableOnly: arch
              DependencyDeprecated: version 1-r2: BDEPEND: cat/deprecated
        "};

        let output = report(FancyReporter::default());
        assert_eq!(expected, &output);
    }

    #[test]
    fn json() {
        let output = report(JsonReporter);
        assert_eq!(REPORTS, &output);
    }

    #[test]
    fn format() {
        let mut format_reporter = FormatReporter::default();

        // empty format string
        let output = report(format_reporter.clone());
        assert_eq!("", &output);

        // existing format strings
        let expected = indoc::indoc! {"
            pkg
            pkg
        "};
        format_reporter.format = "{package}".to_string();
        let output = report(format_reporter.clone());
        assert_eq!(expected, &output);
    }
}
