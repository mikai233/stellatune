use crate::api::{
    error::AppError,
    library::{shared_library_if_initialized, shared_player_service},
};
use anyhow::{Result, anyhow};
use std::sync::{Arc, OnceLock};
use stellatune_backend_api::media_catalog::MediaCatalogService;
use stellatune_library::catalog::{
    CatalogItem, CatalogPage, CatalogQuery, LibrarySource, MediaRef,
};

fn service() -> Result<&'static Arc<MediaCatalogService>> {
    static SERVICE: OnceLock<Arc<MediaCatalogService>> = OnceLock::new();
    if let Some(service) = SERVICE.get() {
        return Ok(service);
    }
    let local =
        shared_library_if_initialized().ok_or_else(|| anyhow!("library not initialized"))?;
    let catalog = MediaCatalogService::new(
        local.handle().catalog().clone(),
        shared_player_service()?,
        stellatune_backend_api::runtime::shared_typescript_runtime(),
    );
    Ok(SERVICE.get_or_init(|| Arc::new(catalog)))
}

pub async fn catalog_list_sources() -> Result<Vec<LibrarySource>, AppError> {
    let result = async { service()?.list_sources().await }.await;
    result.map_err(|e| AppError::capture("catalog_list_sources", e))
}
pub async fn catalog_browse(query: CatalogQuery) -> Result<CatalogPage, AppError> {
    let result = async { service()?.browse(query).await }.await;
    result.map_err(|e| AppError::capture("catalog_browse", e))
}
pub async fn catalog_get_detail(reference: MediaRef) -> Result<CatalogItem, AppError> {
    let result = async { service()?.detail(reference).await }.await;
    result.map_err(|e| AppError::capture("catalog_get_detail", e))
}
pub async fn catalog_prepare_tracks(items: Vec<CatalogItem>) -> Result<Vec<u64>, AppError> {
    let result = async { service()?.prepare_tracks(items).await }.await;
    result.map_err(|e| AppError::capture("catalog_prepare_tracks", e))
}
pub async fn catalog_collect_tracks(
    query: CatalogQuery,
    request_id: String,
) -> Result<Vec<CatalogItem>, AppError> {
    let result = async { service()?.collect_tracks(query, request_id).await }.await;
    result.map_err(|e| AppError::capture("catalog_collect_tracks", e))
}
pub async fn catalog_cancel_collection(request_id: String) -> Result<(), AppError> {
    let result = async {
        service()?.cancel_collection(request_id).await;
        Ok(())
    }
    .await;
    result.map_err(|e| AppError::capture("catalog_cancel_collection", e))
}
