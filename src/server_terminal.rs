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


use axum::{response::IntoResponse, Json};
use serde_json::json;

pub async fn terminal_command(Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let cmd = payload["command"].as_str().unwrap_or("");
    Json(json!({ "type": "output", "data": format!(">> Processing: {}", cmd) }))
}
