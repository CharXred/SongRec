mod auth;
mod docker;
mod error;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use axum_auth::AuthBearer;
use axum_macros::debug_handler;
use error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs, sync::Arc};
use tracing::Level;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub image: String,
    pub endpoint: String,
    pub interval: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RadioStation {
    pub name: String,
    pub url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy(),
        )
        .init();
    tracing::info!("Starting...");

    let config: AppConfig = toml::from_str(&fs::read_to_string("config.toml")?)?;
    let config = Arc::new(config);
    // docker::pull_image(&config.image)?;

    let app = Router::new()
        .route("/", post(create_radio_station))
        .route("/", get(list_radio_stations))
        .route("/:name", delete(delete_radio_station))
        .route("/restart/:name", get(restart_radio_station))
        .route("/restart_all", get(restart_all_radio_stations))
        .route("/pull_image", get(pull_image))
        .with_state(config);

    let address = "127.0.0.1:3000".parse()?;
    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

#[debug_handler]
async fn delete_radio_station(
    Path(name): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<impl IntoResponse, AppError> {
    auth::check(&token)?;
    tracing::info!("Deleting radio: {:?}", name);
    let logs = docker::remove_container(&name)?;
    Ok(Json(json!({ "success": true, "logs": logs })))
}

#[debug_handler]
async fn restart_radio_station(
    Path(name): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<impl IntoResponse, AppError> {
    auth::check(&token)?;
    tracing::info!("Restarting radio: {:?}", name);
    let logs = docker::restart_container(&name)?;
    Ok(Json(json!({ "success": true, "logs": logs })))
}

#[debug_handler]
async fn restart_all_radio_stations(
    AuthBearer(token): AuthBearer,
) -> Result<impl IntoResponse, AppError> {
    auth::check(&token)?;
    tracing::info!("Restarting all the radios");
    let logs = docker::restart_all_containers()?;
    Ok(Json(json!({ "success": true, "logs": logs })))
}

#[debug_handler]
async fn list_radio_stations(AuthBearer(token): AuthBearer) -> Result<impl IntoResponse, AppError> {
    auth::check(&token)?;
    tracing::info!("Listing radios");
    let containers = docker::get_running_containers()?;
    Ok(Json(json!({ "success": true, "radios": containers })))
}

#[debug_handler]
async fn create_radio_station(
    State(config): State<Arc<AppConfig>>,
    AuthBearer(token): AuthBearer,
    Json(station): Json<RadioStation>,
) -> Result<impl IntoResponse, AppError> {
    auth::check(&token)?;
    tracing::info!("Creating radio: {:?}", station);
    docker::create_new_container(&station, &config)?;
    Ok(Json(json!({ "success": true })))
}

#[debug_handler]
async fn pull_image(
    State(config): State<Arc<AppConfig>>,
    AuthBearer(token): AuthBearer,
) -> Result<impl IntoResponse, AppError> {
    auth::check(&token)?;
    tracing::info!("Pulling the latest image");
    docker::pull_image(&config.image)?;
    Ok(Json(json!({ "success": true })))
}
