/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tokio_util::io::ReaderStream;

pub async fn handle_factory_export() -> impl IntoResponse {
    let folder = ".lazarus/factory";
    let output_zip = "nexus_factory_export.zip";

    let _ = std::process::Command::new("zip").args(["-r", "-j", output_zip, folder]).status();

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
