use chalamet_pir::server::Server;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone, Serialize, Deserialize, Debug)]
struct ClientSetupParams {
    seed: [u8; chalamet_pir::SEED_BYTE_LEN],
    hint: Vec<u8>,
    filter: Vec<u8>,
}

/// Handles an individual client connection.
async fn handle_client(mut stream: TcpStream, setup_params: ClientSetupParams, server: Server) {
    println!("New connection from: {}", stream.peer_addr().unwrap());
    let mut buf = vec![0u8; 1024 * 1024];

    loop {
        match stream.read(&mut buf).await {
            Ok(0) => {
                println!("Connection closed");
                break;
            }
            Ok(n) => {
                match n {
                    5 => {
                        let received = String::from_utf8_lossy(&buf[..n]);
                        if received.to_ascii_lowercase() == "setup" {
                            let setup_params_bytes = serde_json::to_vec(&setup_params).unwrap();

                            stream.write_u64_le(setup_params_bytes.len() as u64).await.unwrap_or_else(|e| {
                                eprintln!("Failed to send setup parameters metadata to PIR client: {}", e);
                            });

                            stream.write_all(&setup_params_bytes).await.unwrap_or_else(|e| {
                                eprintln!("Failed to send setup parameters to PIR client: {}", e);
                            });
                        } else {
                            stream.write_all(b"unknown request").await.unwrap_or_else(|e| {
                                eprintln!("Failed to inform client: {}", e);
                            });
                        }
                    }
                    _ => {
                        let start_tm = Instant::now();
                        if let Ok(response) = server.respond(&buf[..n]) {
                            stream.write_u64_le(response.len() as u64).await.unwrap_or_else(|e| {
                                eprintln!("Failed to send response metadata to PIR client: {}", e);
                            });
                            stream.write_all(&response).await.unwrap_or_else(|e| {
                                eprintln!("Failed to send response to client: {}", e);
                            });
                        } else {
                            stream.write_all(b"failed to run PIR query").await.unwrap_or_else(|e| {
                                eprintln!("Failed to inform client: {}", e);
                            });
                        }
                        println!("Responded in {:?}", start_tm.elapsed());
                    }
                };
            }
            Err(e) => {
                eprintln!("Failed to read from client: {}", e);
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path_to_json_file>", args[0]);
        std::process::exit(1);
    }
    let file_path = &args[1];

    let file = File::open(file_path).unwrap_or_else(|err| {
        eprintln!("Error opening file {}: {}", file_path, err);
        std::process::exit(1);
    });
    let reader = BufReader::new(file);

    let deserialized: Map<String, Value> = serde_json::from_reader(reader).unwrap_or_else(|err| {
        eprintln!("Error parsing JSON: {}", err);
        std::process::exit(1);
    });

    println!("Parsing JSON file");

    let start_tm = Instant::now();
    let kv_map: HashMap<Vec<u8>, Vec<u8>> = deserialized
        .into_iter()
        .map(|(k, v)| (k.as_bytes().to_vec(), v.to_string().as_bytes().to_vec()))
        .collect();
    let kv_map_ref = kv_map.iter().map(|(k, v)| (k.as_slice(), v.as_slice())).collect();

    println!("Done in {:?}", start_tm.elapsed());

    let mut rng = ChaCha8Rng::from_os_rng();
    let mut seed_μ = [0u8; chalamet_pir::SEED_BYTE_LEN]; // You'll want to generate a cryptographically secure random seed
    rng.fill_bytes(&mut seed_μ);

    println!("Setting up ChalametPIR server");

    let start_tm = Instant::now();
    let (server, hint_bytes, filter_param_bytes) = Server::setup::<3>(&seed_μ, kv_map_ref).unwrap_or_else(|e| {
        eprintln!("Server setup failed: {}", e);
        std::process::exit(1);
    });

    println!("Done in {:?}", start_tm.elapsed());

    // Bind the TCP listener to an address.
    let listener = TcpListener::bind("127.0.0.1:7878").await.expect("Failed to bind");

    println!("Server listening on 127.0.0.1:7878");

    let client_setup_params = ClientSetupParams {
        seed: seed_μ,
        hint: hint_bytes,
        filter: filter_param_bytes,
    };

    // Accept incoming connections in an infinite loop.
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let movable_client_setup_params = client_setup_params.clone();
                let movable_server_handle = server.clone();

                tokio::spawn(async move {
                    handle_client(stream, movable_client_setup_params, movable_server_handle).await;
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}
