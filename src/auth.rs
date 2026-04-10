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


use anyhow::Result;

/// verify_sovereign_link: Validates a bearer token against the local
/// NEXUS_AUTH_KEY environment variable. Designed for LAN-only use.
pub async fn verify_sovereign_link(token: &str) -> Result<bool> {
    let key = std::env::var("NEXUS_AUTH_KEY").unwrap_or_else(|_| "SOVEREIGN".into());
    Ok(token == key)
}
