use std::time::{Duration, Instant};

use crate::builtins::{make_builtin, ScopedOptions};
use crate::command::Command;
use crate::{Error, ExecStatus};

static LONG_DOC: &str = "Profile a given function or command.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> crate::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    // Disable shell history if history support is enabled, so the command being profiled isn't
    // added, reverting to the previous state on scope exit.
    let mut opts = ScopedOptions::default();
    let _ = opts.disable(["history"]);

    let mut loops = 0;
    let mut elapsed = Duration::new(0, 0);
    let cmd = args.join(" ");
    eprintln!("profiling: {cmd}");

    // force success so the shell doesn't exit prematurely while profiling
    let cmd_str = format!("{cmd} || :");
    let cmd: Command = cmd_str.parse()?;
    let start = Instant::now();
    while elapsed.as_secs() < 3 {
        cmd.execute().ok();
        loops += 1;
        elapsed = start.elapsed();
    }

    let per_loop = elapsed / loops;
    eprintln!("elapsed {elapsed:?}, loops: {loops}, per loop: {per_loop:?}");
    Ok(ExecStatus::Success)
}

make_builtin!("profile", profile_builtin, run, LONG_DOC, "profile func arg1 arg2");

#[cfg(test)]
mod tests {
    use crate::builtins;

    use super::BUILTIN as profile;

    #[test]
    fn builtin() {
        builtins::register([profile]);

        // no args
        assert!(profile(&[]).is_err());

        // basic command
        assert!(profile(&["echo 1"]).is_ok());
    }
}
