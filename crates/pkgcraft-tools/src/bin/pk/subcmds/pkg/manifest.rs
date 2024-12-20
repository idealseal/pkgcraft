use std::fs;
use std::io::{stdout, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};
use clap::builder::ArgPredicate;
use clap::Args;
use futures::{stream, StreamExt};
use indexmap::{IndexMap, IndexSet};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use itertools::Itertools;
use pkgcraft::cli::{pkgs_ebuild, MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::error::Error;
use pkgcraft::fetch::Fetcher;
use pkgcraft::macros::build_path;
use pkgcraft::pkg::{Package, RepoPackage};
use pkgcraft::repo::RepoFormat;
use pkgcraft::traits::{Contains, LogErrors};
use pkgcraft::utils::bounded_jobs;
use rayon::prelude::*;
use tempfile::TempDir;
use tracing::{error, warn};

use super::tokio;

/// Use a specific path or a new temporary directory.
enum PathOrTempdir {
    Path(Utf8PathBuf),
    Tempdir(TempDir),
}

impl PathOrTempdir {
    /// Create a new [`PathOrTempdir`] from an optional path.
    fn new(path: Option<&Utf8Path>) -> anyhow::Result<Self> {
        if let Some(value) = path {
            fs::create_dir_all(value).map_err(|e| anyhow!("failed creating directory: {e}"))?;
            Ok(Self::Path(value.to_path_buf()))
        } else {
            let tmpdir =
                TempDir::new().map_err(|e| anyhow!("failed creating temporary directory: {e}"))?;
            Ok(Self::Tempdir(tmpdir))
        }
    }

    /// Get the [`Utf8Path`] of the chosen location if possible.
    fn as_path(&self) -> anyhow::Result<&Utf8Path> {
        match self {
            Self::Path(path) => Ok(path),
            Self::Tempdir(tmpdir) => Utf8Path::from_path(tmpdir.path())
                .ok_or_else(|| anyhow!("invalid temporary directory")),
        }
    }
}

#[derive(Debug, Args)]
#[clap(next_help_heading = "Target options")]
pub(crate) struct Command {
    /// Concurrent downloads
    #[arg(short, long, default_value = "3")]
    concurrent: usize,

    /// Destination directory
    #[arg(short, long)]
    dir: Option<Utf8PathBuf>,

    /// Force remanifest
    #[arg(short, long)]
    force: bool,

    /// Ignore invalid service certificates
    #[arg(short, long)]
    insecure: bool,

    /// Connection timeout in seconds
    #[arg(short, long, default_value = "15")]
    timeout: f64,

    /// Disable progress output
    #[arg(short, long)]
    no_progress: bool,

    /// Output to stdout
    #[arg(long)]
    stdout: bool,

    /// Target repo
    #[arg(long)]
    repo: Option<String>,

    /// Process fetch-restricted packages
    #[arg(long)]
    restrict: bool,

    /// Force manifest type
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        hide_possible_values = true,
        value_name = "BOOL",
    )]
    thick: Option<bool>,

    // positionals
    /// Target packages or paths
    #[arg(
        // default to the current working directory
        default_value = ".",
        // default to all packages when targeting a repo
        default_value_if("repo", ArgPredicate::IsPresent, Some("*")),
        help_heading = "Arguments",
    )]
    targets: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let concurrent = bounded_jobs(self.concurrent);
        // TODO: pull DISTDIR from config for the default
        let dir = PathOrTempdir::new(self.dir.as_deref())?;
        let dir = dir.as_path()?;

        // convert targets to restrictions
        let targets: Vec<_> = TargetRestrictions::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .targets(self.targets.iter().flatten())
            .try_collect()?;
        config.finalize()?;

        // convert restrictions to pkgs
        let mut iter = pkgs_ebuild(targets).log_errors();

        let failed = &AtomicBool::new(false);
        let mut pkgs: IndexMap<_, IndexSet<_>> = IndexMap::new();
        for pkg in &mut iter {
            let manifest = pkg.manifest();
            let thick = self
                .thick
                .unwrap_or_else(|| !pkg.repo().metadata().config.thin_manifests);

            // A manifest entry is regenerated if its type (thick vs thin) doesn't match
            // the requested setting, the entry hashes don't match the repo hashes, or the
            // related file isn't in the manifest.
            let regen_entry = |name: &str| -> bool {
                if let Some(entry) = manifest.get(name) {
                    manifest.is_thick() != thick
                        || entry
                            .hashes()
                            .keys()
                            .ne(&pkg.repo().metadata().config.manifest_hashes)
                } else {
                    true
                }
            };

            // A manifest is regenerated if its type (thick vs thin) doesn't match
            // the requested setting or the entry hashes don't match the repo hashes.
            let regen = || -> bool {
                manifest.is_thick() != thick
                    || manifest
                        .iter()
                        .flat_map(|e| e.hashes().keys())
                        .any(|hash| !pkg.repo().metadata().config.manifest_hashes.contains(hash))
            };

            if self.restrict || !pkg.restrict().contains("fetch") {
                let mut fetchables = pkg
                    .fetchables()
                    .filter_map(|result| match result {
                        Ok(value) => Some(value),
                        Err(e) => {
                            error!("{e}");
                            failed.store(true, Ordering::Relaxed);
                            None
                        }
                    })
                    .filter(|f| self.force || regen_entry(f.filename()))
                    .peekable();
                if fetchables.peek().is_some() || self.force || regen() {
                    pkgs.entry((pkg.repo(), pkg.cpn().clone(), thick))
                        .or_default()
                        .extend(fetchables.map(|f| {
                            let path = dir.join(f.filename());
                            (f, path)
                        }))
                }
            } else {
                warn!("skipping fetch restricted package: {pkg}");
            }
        }

        let builder = reqwest::Client::builder()
            .danger_accept_invalid_certs(self.insecure)
            .hickory_dns(true)
            .read_timeout(Duration::from_secs_f64(self.timeout))
            .connect_timeout(Duration::from_secs_f64(self.timeout))
            .referer(false);
        let fetcher = &Fetcher::new(builder)?;

        // show a global progress bar when downloading more files than concurrency limit
        let downloads = pkgs.values().flatten().count();
        let global_pb = if downloads > concurrent {
            Some(ProgressBar::new(downloads as u64))
        } else {
            None
        };

        // initialize progress handling
        let mb = &MultiProgress::new();
        let hidden = !stdout().is_terminal() || self.no_progress;
        if hidden {
            mb.set_draw_target(ProgressDrawTarget::hidden());
        } else if let Some(pb) = global_pb.as_ref() {
            mb.add(pb.clone());
        }

        // download files asynchronously tracking failure status
        let global_pb = &global_pb;
        tokio().block_on(async {
            // assume existing files are completely downloaded
            let targets = pkgs.iter().flat_map(|((repo, cpn, _), fetchables)| {
                fetchables.iter().filter_map(move |(f, path)| {
                    if !path.exists() {
                        let pkg_manifest = repo.metadata().pkg_manifest(cpn);
                        let manifest = pkg_manifest.get(f.filename());
                        Some((f, path, manifest.cloned()))
                    } else {
                        None
                    }
                })
            });

            // convert targets into download results stream
            let results = stream::iter(targets)
                .map(|(f, path, manifest)| async move {
                    let size = manifest.as_ref().map(|m| m.size());
                    let part_path = Utf8PathBuf::from(format!("{path}.part"));
                    let result = fetcher.fetch_from_mirrors(f, &part_path, mb, size).await;
                    (result, manifest, part_path, path)
                })
                .buffer_unordered(concurrent);

            // process results stream while logging errors
            results
                .for_each(|(mut result, manifest, src, dest)| async move {
                    // verify file hashes if manifest entry exists
                    if !self.force {
                        if let Some(manifest) = manifest.as_ref() {
                            if result.is_ok() {
                                result = match tokio::fs::read(&src).await {
                                    Ok(data) => manifest.verify(&data),
                                    Err(e) => Err(Error::InvalidValue(format!(
                                        "failed reading: {src}: {e}"
                                    ))),
                                }
                            }
                        }
                    }

                    if let Err(e) = result {
                        mb.suspend(|| error!("{e}"));
                        failed.store(true, Ordering::Relaxed);
                        fs::rename(src, format!("{dest}.failed")).ok();
                    } else {
                        fs::rename(src, dest).ok();
                    }

                    if let Some(pb) = global_pb.as_ref() {
                        pb.inc(1);
                    }
                })
                .await;
        });

        // clear global progress bar
        if let Some(pb) = global_pb.as_ref() {
            pb.finish_and_clear();
        }

        // create manifests if no download failures occurred
        if !failed.load(Ordering::Relaxed) {
            for ((repo, cpn, thick), fetchables) in pkgs {
                let pkgdir = build_path!(&repo, cpn.category(), cpn.package());

                // load manifest from file
                let mut manifest = match repo.metadata().pkg_manifest_parse(&cpn) {
                    Ok(value) => value,
                    Err(e) => {
                        error!("{e}");
                        Default::default()
                    }
                };

                // collect files for hashing
                let distfiles = fetchables.into_par_iter().map(|(_, path)| path);

                // update manifest entries
                let hashes = &repo.metadata().config.manifest_hashes;
                if let Err(e) = manifest.update(distfiles, hashes, &pkgdir, thick) {
                    error!("{e}");
                    failed.store(true, Ordering::Relaxed);
                    continue;
                }

                // write manifest to target output
                let manifest_path = pkgdir.join("Manifest");
                if self.stdout {
                    write!(stdout(), "{manifest}")?;
                } else if !manifest.is_empty() {
                    fs::write(&manifest_path, manifest.to_string())
                        .map_err(|e| anyhow!("{cpn}::{repo}: failed writing manifest: {e}"))?;
                } else if manifest_path.exists() {
                    fs::remove_file(&manifest_path)
                        .map_err(|e| anyhow!("{cpn}::{repo}: failed removing manifest: {e}"))?;
                }
            }
        }

        let status = iter.failed() | failed.load(Ordering::Relaxed);
        Ok(ExitCode::from(status as u8))
    }
}
