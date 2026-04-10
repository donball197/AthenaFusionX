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


use axum::{Router, routing::get, response::IntoResponse, http::HeaderName};
use std::fs;
use std::sync::Arc;
use crate::server::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/ast/summary", get(summary))
        .route("/api/ast/graph", get(graph))
        .route("/api/ast/complexity", get(complexity))
        .route("/api/ast/smells", get(smells))
}

async fn summary() -> impl IntoResponse { serve_json(".nexus_ast/ast_summary.json") }
async fn graph() -> impl IntoResponse { serve_json(".nexus_ast/symbol_graph.json") }
async fn complexity() -> impl IntoResponse { serve_json(".nexus_ast/complexity.json") }
async fn smells() -> impl IntoResponse { serve_json(".nexus_ast/smell_report.json") }

fn serve_json(path: &str) -> ([(HeaderName, &'static str); 1], String) {
    let data = fs::read_to_string(path).unwrap_or_else(|_| "{}".into());
    ([(axum::http::header::CONTENT_TYPE, "application/json")], data)
}
