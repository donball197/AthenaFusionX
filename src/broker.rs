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


// src/broker.rs - v2.0 (Indexes structs, traits, impls — not just functions)
use chrono;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use tokio::sync::RwLock;
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AtlasEntry {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub signature: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ProjectAtlas {
    pub entries: Vec<AtlasEntry>,
    pub timestamp: i64,
}

static CACHED_ATLAS: Lazy<Arc<RwLock<ProjectAtlas>>> =
    Lazy::new(|| Arc::new(RwLock::new(ProjectAtlas::default())));

pub struct KnowledgeBroker;

impl KnowledgeBroker {
    pub async fn refresh_atlas() {
        let mut entries = Vec::new();

        // Match: pub fn, pub async fn
        let re_fn    = Regex::new(r"pub (?:async )?fn ([a-zA-Z0-9_]+)").unwrap();
        // Match: pub struct Foo
        let re_struct = Regex::new(r"pub struct ([a-zA-Z0-9_]+)").unwrap();
        // Match: pub trait Foo
        let re_trait  = Regex::new(r"pub trait ([a-zA-Z0-9_]+)").unwrap();
        // Match: impl Foo or impl<T> Foo
        let re_impl   = Regex::new(r"^impl(?:<[^>]+>)?\s+([a-zA-Z0-9_]+)").unwrap();
        // Match: pub enum Foo
        let re_enum   = Regex::new(r"pub enum ([a-zA-Z0-9_]+)").unwrap();

        let walk_result: Result<Vec<_>, _> = WalkDir::new("src").into_iter().collect();
        if let Ok(entries_raw) = walk_result {
            for entry in entries_raw
                .into_iter()
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
            {
                let path = entry.path().to_string_lossy().to_string();
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    for cap in re_fn.captures_iter(&content) {
                        entries.push(AtlasEntry {
                            name: cap[1].to_string(),
                            kind: "function".into(),
                            path: path.clone(),
                            signature: cap[0].to_string(),
                        });
                    }
                    for cap in re_struct.captures_iter(&content) {
                        entries.push(AtlasEntry {
                            name: cap[1].to_string(),
                            kind: "struct".into(),
                            path: path.clone(),
                            signature: cap[0].to_string(),
                        });
                    }
                    for cap in re_trait.captures_iter(&content) {
                        entries.push(AtlasEntry {
                            name: cap[1].to_string(),
                            kind: "trait".into(),
                            path: path.clone(),
                            signature: cap[0].to_string(),
                        });
                    }
                    for cap in re_enum.captures_iter(&content) {
                        entries.push(AtlasEntry {
                            name: cap[1].to_string(),
                            kind: "enum".into(),
                            path: path.clone(),
                            signature: cap[0].to_string(),
                        });
                    }
                    for line in content.lines() {
                        if let Some(cap) = re_impl.captures(line.trim()) {
                            entries.push(AtlasEntry {
                                name: cap[1].to_string(),
                                kind: "impl".into(),
                                path: path.clone(),
                                signature: line.trim().to_string(),
                            });
                        }
                    }
                }
            }
        }

        let mut cache = CACHED_ATLAS.write().await;
        *cache = ProjectAtlas { entries, timestamp: chrono::Utc::now().timestamp() };
    }

    pub async fn get_atlas() -> ProjectAtlas {
        let cache = CACHED_ATLAS.read().await;
        (*cache).clone()
    }

    pub async fn get_compressed_context() -> String {
        let cache = CACHED_ATLAS.read().await;
        let mut summary = String::from("PROJECT_ATLAS:\n");
        for e in cache.entries.iter().take(50) {
            summary.push_str(&format!("[{}] {} in {}\n", e.kind, e.name, e.path));
        }
        summary
    }
}
