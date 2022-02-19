use std::fmt;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use async_trait::async_trait;
use futures::executor::block_on;
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::error::Error;

#[cfg(feature = "git")]
mod git;
mod tar;

#[derive(Debug, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub(crate) enum Syncer {
    #[cfg(feature = "git")]
    Git(git::Repo),
    TarHttps(tar::Repo),
    Noop,
}

impl fmt::Display for Syncer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            #[cfg(feature = "git")]
            Syncer::Git(repo) => write!(f, "{}", repo.uri),
            Syncer::TarHttps(repo) => write!(f, "{}", repo.uri),
            Syncer::Noop => write!(f, "\"\""),
        }
    }
}

#[async_trait]
pub(self) trait Syncable {
    fn uri_to_syncer(uri: &str) -> crate::Result<Syncer>;
    async fn sync<P: AsRef<Path> + Send>(&self, path: P) -> crate::Result<()>;
}

impl Syncer {
    pub(crate) fn sync<P: AsRef<Path>>(&self, path: P) -> crate::Result<()> {
        let path = path.as_ref();

        // make sure repos dir exists
        let repos_dir = path.parent().unwrap();
        if !repos_dir.exists() {
            fs::create_dir_all(&repos_dir).map_err(|e| {
                Error::RepoSync(format!("failed creating repos dir {repos_dir:?}: {e}"))
            })?;
        }

        match self {
            #[cfg(feature = "git")]
            Syncer::Git(repo) => block_on(repo.sync(path)),
            Syncer::TarHttps(repo) => block_on(repo.sync(path)),
            Syncer::Noop => Ok(()),
        }
    }
}

impl FromStr for Syncer {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[rustfmt::skip]
        let prioritized_syncers = [
            #[cfg(feature = "git")]
            git::Repo::uri_to_syncer,
            tar::Repo::uri_to_syncer,
        ];

        let mut syncer = Syncer::Noop;
        for func in prioritized_syncers.iter() {
            if let Ok(sync) = func(s) {
                syncer = sync;
                break;
            }
        }

        Ok(syncer)
    }
}
