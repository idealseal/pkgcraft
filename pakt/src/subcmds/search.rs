use anyhow::Result;
use clap::{App, AppSettings, ArgMatches};

use crate::settings::Settings;
use crate::Client;

pub fn cmd() -> App<'static> {
    App::new("search")
        .about("search repos")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
}

pub fn run(args: &ArgMatches, _client: &mut Client, _settings: &mut Settings) -> Result<()> {
    let (_subcmd, _m) = args.subcommand().unwrap();
    Ok(())
}
