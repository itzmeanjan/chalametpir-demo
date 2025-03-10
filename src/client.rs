use chalamet_pir::client::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::time::Instant;
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[derive(Clone, Serialize, Deserialize, Debug)]
struct ClientSetupParams {
    seed: [u8; chalamet_pir::SEED_BYTE_LEN],
    hint: Vec<u8>,
    filter: Vec<u8>,
}

#[tokio::main]
async fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:7878").await.expect("❌ Unable to connect to server");
    println!("🎉 Connected to PIR server at 127.0.0.1:7878");

    println!("⏳ Requesting PIR client setup parameters from PIR server...");
    let start_tm = Instant::now();

    if let Err(e) = stream.write_all(b"setup").await {
        eprintln!("❌ Failed to write to stream: {}", e);
        std::process::exit(1);
    }
    println!("✅ Sent PIR client setup parameters request in {:?}", start_tm.elapsed());

    println!("⏳ Receiving PIR client setup parameters...");
    let start_tm = Instant::now();

    let mut len_bytes = [0u8; std::mem::size_of::<u64>()];
    if let Err(e) = stream.read_exact(&mut len_bytes).await {
        eprintln!("❌ Failed to read setup parameters length from stream: {}", e);
        std::process::exit(1);
    }

    let buffer_len = u64::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
    let mut buffer = vec![0; buffer_len];
    let mut bytes_read = 0;

    loop {
        match stream.read(&mut buffer[bytes_read..]).await {
            Ok(0) => break,
            Ok(n) => bytes_read += n,
            Err(e) => {
                eprintln!("❌ Failed to read from stream: {}", e);
                break;
            }
        }
        if bytes_read == buffer.len() {
            break;
        }
    }

    println!("✅ Received PIR client setup parameters of {}B, in {:?}", bytes_read, start_tm.elapsed());
    println!("⏳ Setting up PIR client...");

    let start_tm = Instant::now();
    let client_setup_params: ClientSetupParams = serde_json::from_slice(&buffer).unwrap();

    let mut client = Client::setup(&client_setup_params.seed, &client_setup_params.hint, &client_setup_params.filter).unwrap_or_else(|e| {
        eprintln!("❌ PIR client setup failed: {}", e);
        std::process::exit(1);
    });
    println!("✅ Setup PIR client in {:?}", start_tm.elapsed());
    drop(buffer);

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut input = String::new();

    loop {
        input.clear();
        println!("✍️ Type a message and press Enter (or type 'quit' to exit):");

        match reader.read_line(&mut input).await {
            Ok(0) => {
                println!("❎ EOF reached, exiting.");
                break;
            }
            Ok(_) => {
                let trimmed = input.trim_end().to_lowercase();

                match trimmed.as_str() {
                    "quit" => {
                        println!("❎ Exiting.");
                        break;
                    }
                    query_key => {
                        let query_begins_at = Instant::now();
                        let query_key_as_bytes = query_key.as_bytes();

                        println!("⏳ Preparing PIR query for '{}'...", query_key);
                        let start_tm = Instant::now();

                        match client.query(query_key_as_bytes) {
                            Ok(query) => {
                                println!("✅ PIR query prepared in {:?}", start_tm.elapsed());

                                println!("⤴️ Sending PIR query...");
                                let start_tm = Instant::now();

                                if let Err(e) = stream.write_all(&query).await {
                                    eprintln!("❌ Failed to send PIR query: {}", e);
                                    continue;
                                }
                                println!("✅ PIR query sent in {:?}", start_tm.elapsed());

                                println!("⤵️ Receiving PIR response...");
                                let start_tm = Instant::now();

                                let mut len_bytes = [0u8; std::mem::size_of::<u64>()];
                                if let Err(e) = stream.read_exact(&mut len_bytes).await {
                                    eprintln!("❌ Failed to read response length from stream: {}", e);
                                    continue;
                                }

                                let buffer_len = u64::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
                                let mut buffer = vec![0; buffer_len];
                                let mut bytes_read = 0;

                                loop {
                                    match stream.read(&mut buffer[bytes_read..]).await {
                                        Ok(0) => break,
                                        Ok(n) => bytes_read += n,
                                        Err(e) => {
                                            eprintln!("❌ Failed to read from stream: {}", e);
                                            break;
                                        }
                                    }
                                    if bytes_read == buffer.len() {
                                        break;
                                    }
                                }

                                println!("✅ Received PIR response of {}B, in {:?}", bytes_read, start_tm.elapsed());
                                println!("⏳ Processing PIR response...");

                                let start_tm = Instant::now();

                                match client.process_response(query_key_as_bytes, &buffer) {
                                    Ok(response) => {
                                        let json_obj: Map<String, Value> = serde_json::from_slice(&response).unwrap();
                                        let json_str = serde_json::to_string_pretty(&json_obj).unwrap();

                                        println!("✅ Decoded PIR response for query '{}': {}, in {:?}", query_key, json_str, start_tm.elapsed());
                                    }
                                    Err(e) => {
                                        eprintln!("❌ Failed to process PIR response: {}", e);
                                        continue;
                                    }
                                }

                                println!("✅ Completing query took total time of {:?}", query_begins_at.elapsed());
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to prepare query: {}", e);
                                continue;
                            }
                        }
                    }
                };
            }
            Err(e) => {
                eprintln!("❌ Failed to read from stdin: {}", e);
                break;
            }
        }
    }
}
