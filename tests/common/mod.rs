//! Shared test infrastructure: an in-process fake `ec` Admin gRPC server and a
//! helper that drives the compiled `eccli` binary against it.

use eccli::proto::admin_server::{Admin, AdminServer};
use eccli::proto::{
    AddCandidateRequest, AddElectionRequest, CandidateResponse, ElectionIdRequest,
    ElectionListResponse, ElectionResponse, Empty, GenerateTokensRequest, StatusResponse,
    TokenInfo, TokenListResponse, TokensResponse,
};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

/// Canned Admin service used by the integration tests.
#[derive(Default)]
pub struct FakeAdmin;

fn sample_election(id: &str) -> ElectionResponse {
    ElectionResponse {
        id: id.to_string(),
        name: "Test Election".to_string(),
        start_time: 1_000,
        end_time: 2_000,
        status: "open".to_string(),
        rules_id: "plurality".to_string(),
        rsa_pub_key: "PUBKEY".to_string(),
        created_at: 500,
    }
}

#[tonic::async_trait]
impl Admin for FakeAdmin {
    async fn add_election(
        &self,
        req: Request<AddElectionRequest>,
    ) -> Result<Response<ElectionResponse>, Status> {
        let r = req.into_inner();
        Ok(Response::new(ElectionResponse {
            id: "el-test".to_string(),
            name: r.name,
            start_time: r.start_time,
            end_time: r.end_time,
            status: "open".to_string(),
            rules_id: r.rules_id,
            rsa_pub_key: "PUBKEY".to_string(),
            created_at: 500,
        }))
    }

    async fn add_candidate(
        &self,
        req: Request<AddCandidateRequest>,
    ) -> Result<Response<CandidateResponse>, Status> {
        let r = req.into_inner();
        Ok(Response::new(CandidateResponse {
            id: r.id,
            election_id: r.election_id,
            name: r.name,
        }))
    }

    async fn cancel_election(
        &self,
        _req: Request<ElectionIdRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        Ok(Response::new(StatusResponse {
            success: true,
            message: "Election cancelled".to_string(),
        }))
    }

    async fn get_election(
        &self,
        req: Request<ElectionIdRequest>,
    ) -> Result<Response<ElectionResponse>, Status> {
        let id = req.into_inner().election_id;
        if id == "missing" {
            return Err(Status::not_found("Election not found"));
        }
        Ok(Response::new(sample_election(&id)))
    }

    async fn list_elections(
        &self,
        _req: Request<Empty>,
    ) -> Result<Response<ElectionListResponse>, Status> {
        Ok(Response::new(ElectionListResponse {
            elections: vec![sample_election("el-1"), sample_election("el-2")],
        }))
    }

    async fn generate_registration_tokens(
        &self,
        req: Request<GenerateTokensRequest>,
    ) -> Result<Response<TokensResponse>, Status> {
        let n = req.into_inner().count;
        let tokens = (0..n).map(|i| format!("tok-{i}")).collect();
        Ok(Response::new(TokensResponse { tokens }))
    }

    async fn list_registration_tokens(
        &self,
        _req: Request<ElectionIdRequest>,
    ) -> Result<Response<TokenListResponse>, Status> {
        Ok(Response::new(TokenListResponse {
            tokens: vec![
                TokenInfo {
                    token_id: "aaaa1111".to_string(),
                    used: true,
                },
                TokenInfo {
                    token_id: "bbbb2222".to_string(),
                    used: false,
                },
            ],
        }))
    }
}

/// Start the fake Admin server on an ephemeral port; returns its URL.
pub async fn start_fake() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stream = tokio_stream::wrappers::TcpListenerStream::new(listener);
    tokio::spawn(async move {
        Server::builder()
            .add_service(AdminServer::new(FakeAdmin))
            .serve_with_incoming(stream)
            .await
            .unwrap();
    });
    format!("http://{addr}")
}

/// Start a fake server that requires `authorization: Bearer <token>`.
pub async fn start_fake_with_auth(expected: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stream = tokio_stream::wrappers::TcpListenerStream::new(listener);
    tokio::spawn(async move {
        Server::builder()
            .add_service(AdminServer::with_interceptor(
                FakeAdmin,
                move |req: Request<()>| {
                    let provided = req
                        .metadata()
                        .get("authorization")
                        .and_then(|v| v.to_str().ok());
                    if provided == Some(expected) {
                        Ok(req)
                    } else {
                        Err(Status::unauthenticated("invalid or missing admin token"))
                    }
                },
            ))
            .serve_with_incoming(stream)
            .await
            .unwrap();
    });
    format!("http://{addr}")
}

/// Run the compiled `eccli` binary with the given args and capture its output.
pub async fn run_eccli(args: &[&str]) -> std::process::Output {
    tokio::process::Command::new(env!("CARGO_BIN_EXE_eccli"))
        .args(args)
        .output()
        .await
        .expect("failed to run eccli binary")
}
