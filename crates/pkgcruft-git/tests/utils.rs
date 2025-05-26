use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;

use indexmap::IndexMap;
use regex::Regex;
use tokio::{
    io::AsyncBufReadExt,
    io::BufReader,
    process::{Child, Command},
    time::timeout as timeout_future,
};

static PKGCRUFT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("service listening at: (?P<socket>.+)$").unwrap());

/// Wrapper for a running pkgcruft server process.
pub(crate) struct PkgcruftService {
    service: Child,
    pub(crate) socket: String,
}

impl Drop for PkgcruftService {
    fn drop(&mut self) {
        self.service.start_kill().unwrap();
        self.service.try_wait().unwrap();
    }
}

#[derive(Default)]
pub(crate) struct PkgcruftServiceBuilder {
    socket: String,
    args: Vec<String>,
    env: IndexMap<String, String>,
}

impl PkgcruftServiceBuilder {
    pub(crate) fn new<S: std::fmt::Display>(path: S) -> Self {
        Self {
            socket: "[::]:0".to_string(),
            args: vec![path.to_string()],
            env: Default::default(),
        }
    }

    pub(crate) fn socket<S: std::fmt::Display>(mut self, value: S) -> Self {
        self.socket = value.to_string();
        self
    }

    pub(crate) async fn spawn(self) -> PkgcruftService {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_pkgcruft-gitd"));
        cmd.envs(self.env);
        cmd.args(self.args);
        cmd.args(["--bind", &self.socket]);

        // start detached from the current process while capturing stderr
        let mut service = cmd
            .args(["-vv"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        // wait to report it's running
        let stderr = service.stderr.take().expect("no stderr");
        let f = BufReader::new(stderr);
        let timeout = Duration::from_secs(1);
        let socket = match timeout_future(timeout, f.lines().next_line()).await {
            Ok(Ok(Some(line))) => {
                match PKGCRUFT_RE.captures(&line) {
                    Some(m) => m.name("socket").unwrap().as_str().to_owned(),
                    None => {
                        // try to kill service, but ignore failures
                        service.kill().await.ok();
                        panic!("unknown message: {line}");
                    }
                }
            }
            Err(_) => {
                service.kill().await.ok();
                panic!("timed out");
            }
            Ok(Err(e)) => {
                // unknown IO error
                service.kill().await.ok();
                panic!("{e}");
            }
            Ok(Ok(None)) => {
                service.kill().await.ok();
                panic!("no startup message found");
            }
        };

        PkgcruftService { service, socket }
    }
}
