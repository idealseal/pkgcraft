use scallop::builtins::ExecStatus;

use crate::pkgsh::builtins::einstalldocs::install_docs_from;
use crate::pkgsh::BuildData;

use super::emake_install;

pub(crate) fn src_install(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    emake_install(build)?;
    install_docs_from("DOCS")
}
