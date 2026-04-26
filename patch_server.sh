#!/bin/bash
# Replaces the basic bind with a robust SO_REUSEADDR implementation

sed -i '/let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();/c\
    let socket = tokio::net::TcpSocket::new_v4().unwrap();\
    socket.set_reuseaddr(true).unwrap();\
    socket.bind("0.0.0.0:3000".parse().unwrap()).unwrap();\
    let listener = socket.listen(1024).unwrap();' src/server.rs

echo ">> [SUTURE] server.rs SO_REUSEADDR applied."
