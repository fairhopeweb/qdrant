use std::sync::Arc;
use std::time::{Duration, Instant};

use api::grpc::qdrant::collections_server::Collections;
use api::grpc::qdrant::{
    AliasDescription, ChangeAliases, CollectionOperationResponse, CreateCollection,
    DeleteCollection, GetCollectionInfoRequest, GetCollectionInfoResponse, ListAliasesRequest,
    ListAliasesResponse, ListCollectionAliasesRequest, ListCollectionsRequest,
    ListCollectionsResponse, UpdateCollection,
};
use storage::content_manager::conversions::error_to_status;
use storage::dispatcher::Dispatcher;
use tonic::{Request, Response, Status};

use crate::common::collections::*;
use crate::tonic::api::collections_common::get;

pub struct CollectionsService {
    dispatcher: Arc<Dispatcher>,
}

impl CollectionsService {
    pub fn new(dispatcher: Arc<Dispatcher>) -> Self {
        Self { dispatcher }
    }

    async fn perform_operation<O>(
        &self,
        request: Request<O>,
    ) -> Result<Response<CollectionOperationResponse>, Status>
    where
        O: WithTimeout
            + TryInto<
                storage::content_manager::collection_meta_ops::CollectionMetaOperations,
                Error = Status,
            >,
    {
        let operation = request.into_inner();
        let wait_timeout = operation.wait_timeout();
        let timing = Instant::now();
        let result = self
            .dispatcher
            .submit_collection_meta_op(operation.try_into()?, wait_timeout)
            .await
            .map_err(error_to_status)?;

        let response = CollectionOperationResponse::from((timing, result));
        Ok(Response::new(response))
    }

    async fn list_aliases(
        &self,
        _request: Request<ListAliasesRequest>,
    ) -> Result<Response<ListAliasesResponse>, Status> {
        let timing = Instant::now();
        let aliases = self
            .dispatcher
            .toc()
            .list_aliases()
            .await
            .map(|aliases| aliases.into_iter().map(|alias| alias.into()).collect())
            .map_err(error_to_status)?;
        let response = ListAliasesResponse {
            aliases,
            time: timing.elapsed().as_secs_f64(),
        };
        Ok(Response::new(response))
    }

    async fn list_collection_aliases(
        &self,
        request: Request<ListCollectionAliasesRequest>,
    ) -> Result<Response<ListAliasesResponse>, Status> {
        let timing = Instant::now();
        let ListCollectionAliasesRequest { collection_name } = request.into_inner();
        let aliases = self
            .dispatcher
            .toc()
            .collection_aliases(&collection_name)
            .await
            .map(|aliases| {
                aliases
                    .into_iter()
                    .map(|alias| AliasDescription {
                        alias_name: alias,
                        collection_name: collection_name.clone(),
                    })
                    .collect()
            })
            .map_err(error_to_status)?;
        let response = ListAliasesResponse {
            aliases,
            time: timing.elapsed().as_secs_f64(),
        };
        Ok(Response::new(response))
    }
}

#[tonic::async_trait]
impl Collections for CollectionsService {
    async fn get(
        &self,
        request: Request<GetCollectionInfoRequest>,
    ) -> Result<Response<GetCollectionInfoResponse>, Status> {
        get(self.dispatcher.as_ref(), request.into_inner(), None).await
    }

    async fn list(
        &self,
        _request: Request<ListCollectionsRequest>,
    ) -> Result<Response<ListCollectionsResponse>, Status> {
        let timing = Instant::now();
        let result = do_list_collections(&self.dispatcher).await;

        let response = ListCollectionsResponse::from((timing, result));
        Ok(Response::new(response))
    }

    async fn create(
        &self,
        request: Request<CreateCollection>,
    ) -> Result<Response<CollectionOperationResponse>, Status> {
        self.perform_operation(request).await
    }

    async fn update(
        &self,
        request: Request<UpdateCollection>,
    ) -> Result<Response<CollectionOperationResponse>, Status> {
        self.perform_operation(request).await
    }

    async fn delete(
        &self,
        request: Request<DeleteCollection>,
    ) -> Result<Response<CollectionOperationResponse>, Status> {
        self.perform_operation(request).await
    }

    async fn update_aliases(
        &self,
        request: Request<ChangeAliases>,
    ) -> Result<Response<CollectionOperationResponse>, Status> {
        self.perform_operation(request).await
    }

    async fn list_collection_aliases(
        &self,
        request: Request<ListCollectionAliasesRequest>,
    ) -> Result<Response<ListAliasesResponse>, Status> {
        self.list_collection_aliases(request).await
    }

    async fn list_aliases(
        &self,
        request: Request<ListAliasesRequest>,
    ) -> Result<Response<ListAliasesResponse>, Status> {
        self.list_aliases(request).await
    }
}

trait WithTimeout {
    fn wait_timeout(&self) -> Option<Duration>;
}

macro_rules! impl_with_timeout {
    ($operation:ty) => {
        impl WithTimeout for $operation {
            fn wait_timeout(&self) -> Option<Duration> {
                self.timeout.map(Duration::from_secs)
            }
        }
    };
}

impl_with_timeout!(CreateCollection);
impl_with_timeout!(UpdateCollection);
impl_with_timeout!(DeleteCollection);
impl_with_timeout!(ChangeAliases);
