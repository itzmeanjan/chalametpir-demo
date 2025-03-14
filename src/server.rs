use chalamet_pir::server::Server;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Serialize, Deserialize)]
struct ClientSetupParams {
    seed: [u8; chalamet_pir::SEED_BYTE_LEN],
    hint: Vec<u8>,
    filter: Vec<u8>,
}

async fn read_message_byte_length(stream: &mut TcpStream) -> Option<usize> {
    stream.read_u64().await.ok().map(|v| v as usize)
}

async fn read_message(stream: &mut TcpStream, msg_byte_len: usize) -> Vec<u8> {
    let mut msg_bytes = vec![0u8; msg_byte_len];
    let mut bytes_read = 0;

    loop {
        match stream.read(&mut msg_bytes[bytes_read..]).await {
            Ok(0) => break,
            Ok(n) => bytes_read += n,
            Err(e) => {
                eprintln!("❌ Read {}B from client stream, failed to read any further: {}", bytes_read, e);
                break;
            }
        }
        if bytes_read == msg_byte_len {
            break;
        }
    }

    msg_bytes
}

/// Handles an individual client connection.
async fn handle_client(mut stream: TcpStream, setup_params: Arc<ClientSetupParams>, server: Arc<Server>) {
    let remote_addr = stream.peer_addr().unwrap();
    println!("🎉 New connection from: {}", remote_addr);

    loop {
        let msg_byte_len = read_message_byte_length(&mut stream).await;
        match msg_byte_len {
            Some(0) => {
                println!("❌ Connection closed by {}", remote_addr);
                break;
            }
            Some(n) => {
                match n {
                    5 => {
                        let msg = read_message(&mut stream, n).await;
                        let msg_as_str = String::from_utf8_lossy(&msg);

                        if msg_as_str.to_ascii_lowercase() == "setup" {
                            let start_tm = Instant::now();
                            let setup_params_bytes = serde_json::to_vec(setup_params.as_ref()).unwrap();

                            stream.write_u64_le(setup_params_bytes.len() as u64).await.unwrap_or_else(|e| {
                                eprintln!("❌ Failed to send setup parameters metadata to PIR client: {}", e);
                            });
                            stream.write_all(&setup_params_bytes).await.unwrap_or_else(|e| {
                                eprintln!("❌ Failed to send setup parameters to PIR client: {}", e);
                            });

                            println!("✅ Responded to PIR client setup parameters request in {:?}", start_tm.elapsed());
                        } else {
                            stream.write_all(b"unsupported request").await.unwrap_or_else(|e| {
                                eprintln!("❌ Failed to inform client: {}", e);
                            });
                            println!("✅ Responded to unsupported request");
                        }
                    }
                    _ => {
                        let msg = read_message(&mut stream, n).await;

                        let start_tm = Instant::now();
                        if let Ok(response) = server.respond(&msg) {
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
                };
            }
            None => {
                eprintln!("❌ Failed to receive length of message from client");
                break;
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
    let (server, hint_bytes, filter_param_bytes) = Server::setup::<3>(&seed_μ, kv_map_ref).unwrap_or_else(|e| {
        eprintln!("❌ Server setup failed: {}", e);
        std::process::exit(1);
    });

    println!("✅ Done in {:?}", start_tm.elapsed());

    // Bind the TCP listener to an address.
    let listener = TcpListener::bind("127.0.0.1:7878").await.expect("❌ Failed to setup server");

    println!("👂 Server listening on 127.0.0.1:7878");

    let client_setup_params = ClientSetupParams {
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
