use chalamet_pir::server::Server;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

mod common;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("🔆 Usage: {} <path_to_key_value_db_file.json>", args[0]);
        std::process::exit(1);
    }
    let file_path = &args[1];

    let kv_map = get_kv_db_from_json_file(&file_path);
    let kv_map_ref = kv_map.iter().map(|(k, v)| (k.as_slice(), v.as_slice())).collect();

    let mut rng = ChaCha8Rng::from_os_rng();
    let mut seed_μ = [0u8; chalamet_pir::SEED_BYTE_LEN]; // You'll want to generate a cryptographically secure random seed
    rng.fill_bytes(&mut seed_μ);

    println!("⏳ Setting up ChalametPIR server");

    let start_tm = Instant::now();
    let (server, hint_bytes, filter_param_bytes) = Server::setup::<{ common::ARITY }>(&seed_μ, kv_map_ref).unwrap_or_else(|e| {
        eprintln!("❌ Server setup failed: {}", e);
        std::process::exit(1);
    });

    println!("✅ Done in {:?}", start_tm.elapsed());

    // Bind the TCP listener to an address.
    let listener = TcpListener::bind("127.0.0.1:7878").await.expect("❌ Failed to setup server");

    println!("👂 Server listening on 127.0.0.1:7878");

    let client_setup_params = common::ClientSetupParams {
        seed: seed_μ,
        hint: hint_bytes,
        filter: filter_param_bytes,
    };
    let clonable_client_setup_params = Arc::new(client_setup_params);
    let clonable_server = Arc::new(server);

    // Accept incoming connections in an infinite loop.
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let local_client_setup_params = clonable_client_setup_params.clone();
                let local_server_handle = clonable_server.clone();

                tokio::spawn(async move {
                    handle_client(stream, local_client_setup_params, local_server_handle).await;
                });
            }
            Err(e) => {
                eprintln!("❌ Failed to accept connection: {}", e);
            }
        }
    }
}

fn get_kv_db_from_json_file(file_path: &str) -> HashMap<Vec<u8>, Vec<u8>> {
    let file = File::open(file_path).unwrap_or_else(|err| {
        eprintln!("❌ Error opening JSON database file {}: {}", file_path, err);
        std::process::exit(1);
    });
    let reader = BufReader::new(file);

    let deserialized: Map<String, Value> = serde_json::from_reader(reader).unwrap_or_else(|err| {
        eprintln!("❌ Error parsing JSON database file: {}", err);
        std::process::exit(1);
    });

    println!("⏳ Parsing JSON database file");

    let start_tm = Instant::now();
    let kv_map: HashMap<Vec<u8>, Vec<u8>> = deserialized
        .into_iter()
        .map(|(k, v)| (k.as_bytes().to_vec(), v.to_string().as_bytes().to_vec()))
        .collect();

    println!("✅ Done in {:?}", start_tm.elapsed());
    kv_map
}

/// Handles an individual client connection.
async fn handle_client(mut stream: TcpStream, setup_params: Arc<common::ClientSetupParams>, server: Arc<Server>) -> Option<()> {
    let remote_addr = stream.peer_addr().unwrap();
    println!("🎉 New connection from: {}", remote_addr);

    loop {
        let msg_byte_len = common::read_message_byte_length(&mut stream).await;
        match msg_byte_len {
            Some(0) => {
                println!("❌ Connection closed by {}", remote_addr);
                break;
            }
            Some(n) => {
                match n {
                    5 => {
                        let msg = common::read_message(&mut stream, n).await?;
                        let msg_as_str = String::from_utf8_lossy(&msg);

                        if msg_as_str.to_ascii_lowercase() == "setup" {
                            handle_client_setup_request(&mut stream, setup_params.clone()).await;
                        } else {
                            handle_unrecognized_client_request(&mut stream).await;
                        }
                    }
                    _ => {
                        let msg = common::read_message(&mut stream, n).await?;
                        handle_client_pir_query(&mut stream, server.clone(), &msg).await;
                    }
                };
            }
            None => {
                eprintln!("❌ Failed to receive length of message from client");
                break;
            }
        }
    }

    Some(())
}

async fn handle_client_setup_request(stream: &mut TcpStream, setup_params: Arc<common::ClientSetupParams>) {
    let start_tm = Instant::now();
    let setup_params_bytes = serde_json::to_vec(setup_params.as_ref()).unwrap();

    stream.write_u64_le(setup_params_bytes.len() as u64).await.unwrap_or_else(|e| {
        eprintln!("❌ Failed to send client setup parameters metadata to PIR client: {}", e);
    });
    stream.write_all(&setup_params_bytes).await.unwrap_or_else(|e| {
        eprintln!("❌ Failed to send client setup parameters to PIR client: {}", e);
    });

    println!("✅ Responded to PIR client setup parameters request in {:?}", start_tm.elapsed());
}

async fn handle_unrecognized_client_request(stream: &mut TcpStream) {
    stream.write_all(b"unrecognized request").await.unwrap_or_else(|e| {
        eprintln!("❌ Failed to inform client: {}", e);
    });
    println!("✅ Responded to unrecognized request");
}

async fn handle_client_pir_query(stream: &mut TcpStream, server: Arc<Server>, query: &[u8]) {
    let start_tm = Instant::now();
    if let Ok(response) = server.respond(query) {
        stream.write_u64_le(response.len() as u64).await.unwrap_or_else(|e| {
            eprintln!("❌ Failed to send response metadata to PIR client: {}", e);
        });
        stream.write_all(&response).await.unwrap_or_else(|e| {
            eprintln!("❌ Failed to send response to client: {}", e);
        });
    } else {
        stream.write_all(b"failed to run PIR query").await.unwrap_or_else(|e| {
            eprintln!("❌ Failed to inform client: {}", e);
        });
    }
    println!("✅ Responded to PIR query in {:?}", start_tm.elapsed());
}
