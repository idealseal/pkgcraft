use std::fs;
use std::os::fd::AsRawFd;

use scallop::pool::redirect_output;
use scallop::{functions, Error, ExecStatus};
use tempfile::NamedTempFile;

use crate::error::PackageError;
use crate::pkg::{ebuild, Build, Package, Pretend, Regen, Source};
use crate::shell::metadata::Metadata;
use crate::shell::scope::Scope;
use crate::shell::{get_build_mut, BuildData};

use super::OperationKind;

impl<'a> Build for ebuild::Pkg<'a> {
    fn build(&self) -> scallop::Result<()> {
        get_build_mut()
            .source_ebuild(&self.abspath())
            .map_err(|e| self.invalid_pkg_err(e))?;

        for phase in self.eapi().operation(OperationKind::Build)? {
            phase.run().map_err(|e| self.pkg_err(e))?;
        }

        Ok(())
    }
}

impl<'a> Pretend for ebuild::Pkg<'a> {
    fn pretend(&self) -> scallop::Result<Option<String>> {
        let Ok(op) = self.eapi().operation(OperationKind::Pretend) else {
            // ignore packages with EAPIs lacking pkg_pretend() support
            return Ok(None);
        };

        let phase = op.phases[0];

        if !self.defined_phases().contains(phase.short_name()) {
            // phase function is undefined
            return Ok(None);
        }

        self.source()?;

        let Some(mut func) = functions::find(phase) else {
            return Err(Error::Base(format!("{self}: {phase} phase missing")));
        };

        let build = get_build_mut();
        build.scope = Scope::Phase(phase.into());

        // initialize phase scope variables
        build.set_vars()?;

        // redirect pkg_pretend() output to a temporary file
        let file = NamedTempFile::new()?;
        redirect_output(file.as_raw_fd())?;

        // execute function capturing output
        let result = func.execute(&[]);
        let output = fs::read_to_string(file.path()).unwrap_or_default();
        let output = output.trim();

        if let Err(e) = result {
            if output.is_empty() {
                Err(Error::Base(format!("{self}: {e}")))
            } else {
                Err(Error::Base(format!("{self}: {e}\n{output}")))
            }
        } else if !output.is_empty() {
            Ok(Some(format!("{self}\n{output}")))
        } else {
            Ok(None)
        }
    }
}

impl<'a> Source for ebuild::raw::Pkg<'a> {
    fn source(&self) -> scallop::Result<ExecStatus> {
        BuildData::from_raw_pkg(self);
        get_build_mut().source_ebuild(self.data())
    }
}

impl<'a> Source for ebuild::Pkg<'a> {
    fn source(&self) -> scallop::Result<ExecStatus> {
        BuildData::from_pkg(self);
        get_build_mut().source_ebuild(&self.abspath())
    }
}

impl<'a> Regen for ebuild::raw::Pkg<'a> {
    fn regen(&self) -> scallop::Result<()> {
        Ok(Metadata::serialize(self).map_err(|e| self.invalid_pkg_err(e))?)
    }
}

#[cfg(test)]
mod tests {
    use crate::test::TEST_DATA;

    use super::*;

    #[test]
    fn pretend() {
        // no pkg_pretend phase exists
        let pkg = TEST_DATA.ebuild_pkg("pkg-pretend/none::phases").unwrap();
        assert!(pkg.pretend().is_ok());

        // success
        let pkg = TEST_DATA.ebuild_pkg("pkg-pretend/success::phases").unwrap();
        assert!(pkg.pretend().is_ok());

        // success with output
        let pkg = TEST_DATA
            .ebuild_pkg("pkg-pretend/success-with-output::phases")
            .unwrap();
        assert!(pkg.pretend().is_ok());

        // failure
        let pkg = TEST_DATA.ebuild_pkg("pkg-pretend/failure::phases").unwrap();
        assert!(pkg.pretend().is_err());

        // failure with output
        let pkg = TEST_DATA
            .ebuild_pkg("pkg-pretend/failure-with-output::phases")
            .unwrap();
        assert!(pkg.pretend().is_err());
    }
}
