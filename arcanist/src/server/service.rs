use std::sync::Arc;

use pkgcraft::Error;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::settings::Settings;

pub mod arcanist {
    tonic::include_proto!("arcanist");
}

pub use arcanist::arcanist_server::ArcanistServer;
use arcanist::{
    arcanist_server::Arcanist, AddRepoRequest, ListRequest, ListResponse, StringRequest,
    StringResponse,
};

#[derive(Debug)]
pub struct ArcanistService {
    pub settings: Arc<RwLock<Settings>>,
}

#[tonic::async_trait]
impl Arcanist for ArcanistService {
    async fn add_repo(
        &self,
        request: Request<AddRepoRequest>,
    ) -> Result<Response<StringResponse>, Status> {
        let req = request.into_inner();
        let repos = &mut self.settings.write().await.config.repos;
        match repos.add(&req.name, &req.uri) {
            Err(Error::Config(e)) => Err(Status::failed_precondition(&e)),
            Err(e) => Err(Status::internal(format!("{}", &e))),
            Ok(_) => {
                let reply = StringResponse { data: req.name };
                Ok(Response::new(reply))
            }
        }
    }

    async fn remove_repos(
        &self,
        request: Request<ListRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let req = request.into_inner();
        let repos = &mut self.settings.write().await.config.repos;
        match repos.del(&req.data, true) {
            Err(Error::Config(e)) => Err(Status::failed_precondition(&e)),
            Err(e) => Err(Status::internal(format!("{}", &e))),
            Ok(_) => {
                let reply = ListResponse { data: req.data };
                Ok(Response::new(reply))
            }
        }
    }

    async fn list_repos(
        &self,
        _request: Request<StringRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let mut repos: Vec<String> = Vec::new();
        let settings = self.settings.read().await;
        for (id, config) in settings.config.repos.configs.iter() {
            repos.push(format!("{}: {:?}", id, config.location));
        }
        let reply = ListResponse { data: repos };
        Ok(Response::new(reply))
    }

    async fn create_repo(
        &self,
        request: Request<StringRequest>,
    ) -> Result<Response<StringResponse>, Status> {
        let req = request.into_inner();
        let repos = &mut self.settings.write().await.config.repos;
        match repos.create(&req.data) {
            Err(Error::Config(e)) => Err(Status::failed_precondition(&e)),
            Err(e) => Err(Status::internal(format!("{}", &e))),
            Ok(_) => {
                let reply = StringResponse { data: req.data };
                Ok(Response::new(reply))
            }
        }
    }

    async fn sync_repos(
        &self,
        request: Request<ListRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let req = request.into_inner();
        let config = &mut self.settings.write().await.config;
        match config.repos.sync(req.data.clone()) {
            Err(Error::Config(e)) => Err(Status::failed_precondition(&e)),
            Err(e) => Err(Status::internal(format!("{}", &e))),
            Ok(_) => {
                let reply = ListResponse { data: req.data };
                Ok(Response::new(reply))
            }
        }
    }

    type SearchPackagesStream = ReceiverStream<Result<StringResponse, Status>>;

    async fn search_packages(
        &self,
        request: Request<ListRequest>,
    ) -> Result<Response<Self::SearchPackagesStream>, Status> {
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            for pkg in request.into_inner().data.iter() {
                tx.send(Ok(StringResponse {
                    data: pkg.to_string(),
                }))
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
        unimplemented!()
    }

    type RemovePackagesStream = ReceiverStream<Result<StringResponse, Status>>;

    async fn remove_packages(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<Self::RemovePackagesStream>, Status> {
        unimplemented!()
    }

    async fn version(
        &self,
        request: Request<StringRequest>,
    ) -> Result<Response<StringResponse>, Status> {
        let version = format!("{}-{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        let req = request.into_inner();
        let reply = StringResponse {
            data: format!("client: {}, server: {}", req.data, version),
        };
        Ok(Response::new(reply))
    }
}
