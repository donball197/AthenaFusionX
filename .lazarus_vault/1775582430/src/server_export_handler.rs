use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
use tokio_util::io::ReaderStream;

pub async fn handle_factory_export() -> impl IntoResponse {
    let folder = ".lazarus/factory";
    let output_zip = "nexus_factory_export.zip";

    let _ = std::process::Command::new("zip")
        .args(["-r", "-j", output_zip, folder])
        .status();

    let file = match tokio::fs::File::open(output_zip).await {
        Ok(f) => f,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Export Failed").into_response(),
    };

    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    Response::builder()
        .header("Content-Type", "application/zip")
        .header("Content-Disposition", "attachment; filename=\"nexus_swarm_factory.zip\"")
        .body(body)
        .unwrap()
}
