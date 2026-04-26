#!/bin/bash
# Replaces the blocking std::fs::read_dir with tokio::fs::read_dir
sed -i '/\.route("\/fs\/list", get(|Query(q): Query<FsQuery>| async move {/,/}))/c\
        .route("/fs/list", get(|Query(q): Query<FsQuery>| async move {\
            let path = q.path.unwrap_or_else(|| ".".into());\
            let mut entries = Vec::new();\
            if let Ok(mut read_dir) = tokio::fs::read_dir(&path).await {\
                while let Ok(Some(entry)) = read_dir.next_entry().await {\
                    let meta = entry.metadata().await.ok();\
                    entries.push(serde_json::json!({\
                        "name": entry.file_name().to_string_lossy(),\
                        "path": entry.path(),\
                        "is_dir": meta.map(|m| m.is_dir()).unwrap_or(false)\
                    }));\
                }\
            }\
            Json(entries)\
        }))' src/server.rs
