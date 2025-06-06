use std::sync::Arc;

use pkgcraft::config::Config as PkgcraftConfig;
use pkgcraft::repo::Repository;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::settings::Settings;

use arcanist::proto::{
    ListRequest, ListResponse, StringRequest, StringResponse, arcanist_server::Arcanist,
};

#[derive(Debug)]
pub struct ArcanistService {
    #[allow(dead_code)]
    pub settings: Settings,
    pub config: Arc<RwLock<PkgcraftConfig>>,
}

#[tonic::async_trait]
impl Arcanist for ArcanistService {
    async fn list_repos(
        &self,
        _request: Request<StringRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let mut repos = vec![];
        let config = self.config.read().await;
        for (id, repo) in config.repos() {
            repos.push(format!("{id}: {:?}", repo.path()));
        }
        let reply = ListResponse { data: repos };
        Ok(Response::new(reply))
    }

    type SearchPackagesStream = ReceiverStream<Result<StringResponse, Status>>;

    async fn search_packages(
        &self,
        request: Request<ListRequest>,
    ) -> Result<Response<Self::SearchPackagesStream>, Status> {
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            for pkg in request.into_inner().data {
                tx.send(Ok(StringResponse { data: pkg.to_string() }))
                    .await
                    .unwrap();
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type AddPackagesStream = ReceiverStream<Result<StringResponse, Status>>;

    async fn add_packages(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<Self::AddPackagesStream>, Status> {
        todo!()
    }

    type RemovePackagesStream = ReceiverStream<Result<StringResponse, Status>>;

    async fn remove_packages(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<Self::RemovePackagesStream>, Status> {
        todo!()
    }

    async fn version(
        &self,
        request: Request<StringRequest>,
    ) -> Result<Response<StringResponse>, Status> {
        let version = format!("{}-{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        let req = request.into_inner();
        let reply = StringResponse {
            data: format!("client: {}, server: {version}", req.data),
        };
        Ok(Response::new(reply))
    }
}
