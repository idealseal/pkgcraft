use scallop::{Error, ExecStatus};

use super::make_builtin;

static LONG_DOC: &str = "Error out on direct phase function calls";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> scallop::Result<ExecStatus> {
    Err(Error::Base("direct phase call".to_string()))
}

make_builtin!(
    "pkg_config",
    pkg_config_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    PKG_CONFIG_BUILTIN
);

make_builtin!(
    "pkg_info",
    pkg_info_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    PKG_INFO_BUILTIN
);

make_builtin!(
    "pkg_nofetch",
    pkg_nofetch_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    PKG_NOFETCH_BUILTIN
);

make_builtin!(
    "pkg_postinst",
    pkg_postinst_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    PKG_POSTINST_BUILTIN
);

make_builtin!(
    "pkg_postrm",
    pkg_postrm_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    PKG_POSTRM_BUILTIN
);

make_builtin!(
    "pkg_preinst",
    pkg_preinst_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    PKG_PREINST_BUILTIN
);

make_builtin!(
    "pkg_prerm",
    pkg_prerm_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    PKG_PRERM_BUILTIN
);

make_builtin!(
    "pkg_pretend",
    pkg_pretend_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    PKG_PRETEND_BUILTIN
);

make_builtin!(
    "pkg_setup",
    pkg_setup_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    PKG_SETUP_BUILTIN
);

make_builtin!(
    "src_compile",
    src_compile_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    SRC_COMPILE_BUILTIN
);

make_builtin!(
    "src_configure",
    src_configure_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    SRC_CONFIGURE_BUILTIN
);

make_builtin!(
    "src_install",
    src_install_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    SRC_INSTALL_BUILTIN
);

make_builtin!(
    "src_prepare",
    src_prepare_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    SRC_PREPARE_BUILTIN
);

make_builtin!(
    "src_test",
    src_test_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    SRC_TEST_BUILTIN
);

make_builtin!(
    "src_unpack",
    src_unpack_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    SRC_UNPACK_BUILTIN
);
