//! VectorService gRPC implementation.
//!
//! This module provides the gRPC service handlers for vector operations.

use crate::error::IntoStatus;
use crate::id_mapper::to_vector_id;
use crate::pb::vector_service_server::VectorService;
use crate::pb::{
    BatchGetRequest, BatchGetResponse, DeleteRequest, DeleteResponse, GetRequest, GetResponse,
    SearchRequest, SearchResponse, SearchResult, UpsertRequest, UpsertResponse,
};
use std::sync::Arc;
use std::time::Instant;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};
use tracing::{info, warn};
use vajra_common::types::DistanceMetric;
use vajra_common::VajraError;
use vajra_engine::{distance::create_distance_function, HnswIndex};

/// VectorService implementation.
pub struct VectorServiceImpl {
    /// The HNSW index for vector storage.
    index: Arc<HnswIndex>,
    /// Default ef (search width) for queries.
    default_ef: usize,
}

impl VectorServiceImpl {
    /// Create a new VectorService with the given index.
    pub fn new(index: Arc<HnswIndex>, default_ef: usize) -> Self {
        Self { index, default_ef }
    }

    /// Create a test index for development.
    pub fn new_test(dimension: usize, max_vectors: usize) -> Self {
        let config = vajra_common::config::HnswConfig::default();
        let index = HnswIndex::new(
            config,
            dimension,
            max_vectors,
            create_distance_function(DistanceMetric::Euclidean),
        );
        Self {
            index: Arc::new(index),
            default_ef: 50,
        }
    }
}

#[tonic::async_trait]
impl VectorService for VectorServiceImpl {
    /// Streaming upsert for bulk operations.
    #[tracing::instrument(skip(self, request))]
    async fn stream_upsert(
        &self,
        request: Request<Streaming<UpsertRequest>>,
    ) -> Result<Response<UpsertResponse>, Status> {
        let mut stream = request.into_inner();
        let mut upserted_count = 0u64;
        let mut failed_ids = Vec::new();

        while let Some(req) = stream.next().await {
            match req {
                Ok(upsert_req) => {
                    let id = to_vector_id(&upsert_req.id);

                    match self.index.insert(id, upsert_req.vector) {
                        Ok(()) => {
                            upserted_count += 1;
                        }
                        Err(e) => {
                            warn!(id = %upsert_req.id, error = %e, "Upsert failed");
                            failed_ids.push(upsert_req.id);
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Stream error during upsert");
                    return Err(Status::internal(format!("Stream error: {}", e)));
                }
            }
        }

        info!(upserted = upserted_count, failed = failed_ids.len(), "StreamUpsert complete");

        Ok(Response::new(UpsertResponse {
            upserted_count,
            failed_ids,
        }))
    }

    /// Search for nearest neighbors.
    #[tracing::instrument(skip(self, request))]
    async fn search(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<Response<SearchResponse>, Status> {
        let start = Instant::now();
        let req = request.into_inner();

        let k = req.k as usize;
        let ef = if req.ef > 0 {
            req.ef as usize
        } else {
            self.default_ef
        };

        let results = self
            .index
            .search(&req.query, k, ef)
            .map_err(|e: VajraError| e.into_status())?;

        let proto_results: Vec<SearchResult> = results
            .into_iter()
            .map(|r| SearchResult {
                id: format!("{:016x}", r.id.0), // Hex string for client
                score: r.score,
                metadata: Vec::new(), // TODO: fetch metadata if requested
            })
            .collect();

        let latency_us = start.elapsed().as_micros() as u64;

        Ok(Response::new(SearchResponse {
            results: proto_results,
            latency_us,
        }))
    }

    /// Delete a vector.
    #[tracing::instrument(skip(self, request))]
    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let id = to_vector_id(&req.id);

        match self.index.delete(id) {
            Ok(()) => Ok(Response::new(DeleteResponse { deleted: true })),
            Err(VajraError::VectorNotFound { .. }) => {
                Ok(Response::new(DeleteResponse { deleted: false }))
            }
            Err(e) => Err(e.into_status()),
        }
    }

    /// Get a vector by ID.
    #[tracing::instrument(skip(self, request))]
    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let req = request.into_inner();
        let id = to_vector_id(&req.id);

        match self.index.get_vector(id) {
            Some(vector) => {
                let vec_data: Vec<f32> = if req.include_vector {
                    vector.to_vec()
                } else {
                    Vec::new()
                };

                Ok(Response::new(GetResponse {
                    id: req.id,
                    vector: vec_data,
                    metadata: Vec::new(), // TODO: implement metadata storage
                    found: true,
                }))
            }
            None => Ok(Response::new(GetResponse {
                id: req.id,
                vector: Vec::new(),
                metadata: Vec::new(),
                found: false,
            })),
        }
    }

    /// Batch get multiple vectors.
    #[tracing::instrument(skip(self, request))]
    async fn batch_get(
        &self,
        request: Request<BatchGetRequest>,
    ) -> Result<Response<BatchGetResponse>, Status> {
        let req = request.into_inner();

        let mut vectors = Vec::with_capacity(req.ids.len());

        for client_id in req.ids {
            let id = to_vector_id(&client_id);

            let response = match self.index.get_vector(id) {
                Some(vector) => {
                    let vec_data: Vec<f32> = if req.include_vectors {
                        vector.to_vec()
                    } else {
                        Vec::new()
                    };

                    GetResponse {
                        id: client_id,
                        vector: vec_data,
                        metadata: Vec::new(),
                        found: true,
                    }
                }
                None => GetResponse {
                    id: client_id,
                    vector: Vec::new(),
                    metadata: Vec::new(),
                    found: false,
                },
            };

            vectors.push(response);
        }

        Ok(Response::new(BatchGetResponse { vectors }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_search_empty() {
        let service = VectorServiceImpl::new_test(4, 1000);

        let request = Request::new(SearchRequest {
            query: vec![1.0, 2.0, 3.0, 4.0],
            k: 10,
            ef: 50,
            filter: String::new(),
        });

        let result = service.search(request).await;

        // Should return FailedPrecondition for empty index
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn test_service_get_not_found() {
        let service = VectorServiceImpl::new_test(4, 1000);

        let request = Request::new(GetRequest {
            id: "nonexistent".to_string(),
            include_vector: true,
            include_metadata: false,
        });

        let result = service.get(request).await.unwrap();
        assert!(!result.into_inner().found);
    }
}
