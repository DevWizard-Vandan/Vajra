use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};
use vajra_common::types::SearchResult;

use crate::reactor::ClientRequest;

#[derive(Clone)]
pub struct AppState {
    pub client_tx: mpsc::Sender<ClientRequest>,
}

pub async fn start_http_server(addr: SocketAddr, client_tx: mpsc::Sender<ClientRequest>) {
    let state = AppState { client_tx };

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/upsert", post(upsert_handler))
        .route("/search", post(search_handler))
        .with_state(state);

    info!("Starting HTTP REST server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind HTTP REST API address");
    
    if let Err(e) = axum::serve(listener, app).await {
        error!(error = %e, "HTTP REST server error");
    }
}

#[derive(Deserialize)]
pub struct UpsertReq {
    pub id: String,
    pub vector: Vec<f32>,
}

#[derive(Serialize)]
pub struct UpsertResp {
    pub index: u64,
}

pub async fn upsert_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpsertReq>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let req = ClientRequest::Insert {
        id: payload.id,
        vector: payload.vector,
        response: tx,
    };

    if state.client_tx.send(req).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Server overloaded or dead").into_response();
    }

    match rx.await {
        Ok(Ok(index)) => (StatusCode::OK, Json(UpsertResp { index })).into_response(),
        Ok(Err(e)) => {
            error!(error = %e, "Failed to upsert");
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to receive response").into_response(),
    }
}

#[derive(Deserialize)]
pub struct SearchReq {
    pub query: Vec<f32>,
    pub k: Option<usize>,
    pub ef: Option<usize>,
}

#[derive(Serialize)]
pub struct SearchResp {
    pub results: Vec<SearchResult>,
}

pub async fn search_handler(
    State(state): State<AppState>,
    Json(payload): Json<SearchReq>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let req = ClientRequest::Search {
        query: payload.query,
        k: payload.k.unwrap_or(10),
        ef: payload.ef.unwrap_or(50),
        response: tx,
    };

    if state.client_tx.send(req).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Server overloaded or dead").into_response();
    }

    match rx.await {
        Ok(Ok(results)) => (StatusCode::OK, Json(SearchResp { results })).into_response(),
        Ok(Err(e)) => {
            error!(error = %e, "Failed to search");
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to receive response").into_response(),
    }
}

#[derive(Serialize)]
pub struct HealthResp {
    pub status: &'static str,
    pub role: String,
    pub term: u64,
    pub leader: Option<u64>,
    pub vector_count: usize,
    pub last_applied: u64,
}

pub async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let req = ClientRequest::Status { response: tx };

    if state.client_tx.send(req).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Server overloaded or dead").into_response();
    }

    match rx.await {
        Ok(status) => {
            let resp = HealthResp {
                status: "ok",
                role: format!("{:?}", status.role),
                term: status.term,
                leader: status.leader.map(|id| id.get()),
                vector_count: status.vector_count,
                last_applied: status.last_applied,
            };
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to receive response").into_response(),
    }
}
