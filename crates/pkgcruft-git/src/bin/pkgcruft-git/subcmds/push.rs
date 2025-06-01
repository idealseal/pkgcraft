use std::io::{self, BufRead, IsTerminal};
use std::ops::Deref;

use anyhow::anyhow;
use itertools::Itertools;
use pkgcruft::report::Report;
use pkgcruft::reporter::{FancyReporter, Reporter};
use pkgcruft_git::proto::PushRequest;

use crate::Client;

#[derive(clap::Args)]
pub(crate) struct Command {}

impl Command {
    pub(super) async fn run(&self, client: &mut Client) -> anyhow::Result<()> {
        let mut stdout = io::stdout().lock();
        let stdin = io::stdin().lock();
        if stdin.is_terminal() {
            anyhow::bail!("requires running as a git pre-receive hook");
        }

        // pull object directories from the environment
        //
        // git2::Repository::open_from_env() doesn't appear to respect the temporary
        // object directory used for incoming objects before they're merged into the tree
        // so we manually add them ourselves.
        let mut odb_paths = vec![];
        if let Ok(value) = std::env::var("GIT_OBJECT_DIRECTORY") {
            odb_paths.push(value);
        }
        if let Ok(values) = std::env::var("GIT_ALTERNATE_OBJECT_DIRECTORIES") {
            odb_paths.extend(values.split(':').map(|s| s.to_string()));
        }

        // WARNING: This appears to invalidate the environment in some fashion so
        // std::env::var() calls don't work as expected after it even though
        // std::env::vars() will still show all the variables.
        //
        // open git repo specified by $GIT_DIR
        let repo = git2::Repository::open_from_env()
            .map_err(|e| anyhow!("failed opening git repo: {e}"))?;

        // manually add all object directories so incoming commits can be found
        let odb = repo.odb()?;
        for path in odb_paths {
            odb.add_disk_alternate(&path)?;
        }

        let mut failed = false;
        let mut reporter: Reporter = FancyReporter::default().into();

        for line in stdin.lines() {
            let line = line?;
            // TODO: skip pushes where the ref name doesn't match the default branch
            //
            // get push information
            let Some((old_ref, new_ref, ref_name)) = line.split(' ').collect_tuple() else {
                anyhow::bail!("invalid pre-receive hook arguments: {line}");
            };

            // TODO: Consider streaming packfile entries to the server instead of
            // building it in a memory buffer and serializing it.
            //
            // serialize target commits into a packfile
            let mut pack_builder = repo
                .packbuilder()
                .map_err(|e| anyhow!("failed initializing pack builder: {e}"))?;
            let mut revwalk = repo
                .revwalk()
                .map_err(|e| anyhow!("failed creating revwalk: {e}"))?;
            revwalk
                .push_range(&format!("{old_ref}..{new_ref}"))
                .map_err(|e| anyhow!("failed limiting revwalk: {e}"))?;
            pack_builder
                .insert_walk(&mut revwalk)
                .map_err(|e| anyhow!("failed targeting pack builder: {e}"))?;
            let mut buf = git2::Buf::new();
            pack_builder
                .write_buf(&mut buf)
                .map_err(|e| anyhow!("failed serializing packfile: {e}"))?;

            // send request to server
            let push = PushRequest {
                old_ref: old_ref.to_string(),
                new_ref: new_ref.to_string(),
                ref_name: ref_name.to_string(),
                pack: buf.deref().to_vec(),
            };
            let request = tonic::Request::new(push);
            let response = client.push(request).await?;
            let response = response.into_inner();
            failed |= response.failed;

            // output reports
            for report in response.reports {
                let report = Report::from_json(&report)?;
                reporter.report(&report, &mut stdout)?;
            }
        }

        if failed {
            anyhow::bail!("scanning errors found")
        } else {
            Ok(())
        }
    }
}
