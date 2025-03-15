use chalamet_pir::client::Client;
use serde_json::{Map, Value};
use std::time::Instant;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

mod common;

async fn request_setup_parameters(stream: &mut TcpStream) {
    println!("⏳ Requesting PIR client setup parameters from PIR server...");
    let start_tm = Instant::now();

    let setup_msg = b"setup";
    stream.write_u64_le(setup_msg.len() as u64).await.unwrap_or_else(|e| {
        eprintln!("❌ Failed to send setup request metadata: {}", e);
        std::process::exit(1);
    });
    stream.write_all(setup_msg).await.unwrap_or_else(|e| {
        eprintln!("❌ Failed to send setup request: {}", e);
        std::process::exit(1);
    });

    println!("✅ Sent PIR client setup parameters request in {:?}", start_tm.elapsed());
}

async fn receive_setup_parameters(stream: &mut TcpStream) -> Option<Vec<u8>> {
    println!("⏳ Receiving PIR client setup parameters...");
    let start_tm = Instant::now();

    let msg_byte_len = common::read_message_byte_length(stream).await?;
    let setup_params_as_bytes = common::read_message(stream, msg_byte_len).await?;

    println!("✅ Received PIR client setup parameters in {:?}", start_tm.elapsed());
    Some(setup_params_as_bytes)
}

fn setup_pir_client(setup_params_as_bytes: Vec<u8>) -> Client {
    println!("⏳ Setting up PIR client...");

    let start_tm = Instant::now();
    let client_setup_params: common::ClientSetupParams = serde_json::from_slice(&setup_params_as_bytes).unwrap();

    let client = Client::setup(&client_setup_params.seed, &client_setup_params.hint, &client_setup_params.filter).unwrap_or_else(|e| {
        eprintln!("❌ PIR client setup failed: {}", e);
        std::process::exit(1);
    });

    println!("✅ Setup PIR client in {:?}", start_tm.elapsed());
    client
}

async fn handle_pir_query_for_key(stream: &mut TcpStream, pir_client: &mut Client, query_key: &str) {
    let query_begins_at = Instant::now();
    let query_key_as_bytes = query_key.as_bytes();

    println!("⏳ Preparing PIR query for keyword '{}'...", query_key);
    let start_tm = Instant::now();

    match pir_client.query(query_key_as_bytes) {
        Ok(query) => {
            println!("✅ PIR query prepared in {:?}", start_tm.elapsed());

            println!("⤴️ Sending PIR query...");
            let start_tm = Instant::now();

            stream.write_u64_le(query.len() as u64).await.expect("❌ Failed to send PIR query metadata");
            stream.write_all(&query).await.expect("❌ Failed to send PIR query");

            println!("✅ PIR query sent in {:?}", start_tm.elapsed());

            println!("⤵️ Receiving PIR response...");
            let start_tm = Instant::now();

            let response_byte_len = common::read_message_byte_length(stream)
                .await
                .expect("❌ Failed to read response length from stream");
            let response = common::read_message(stream, response_byte_len)
                .await
                .expect("❌ Failed to read PIR query response");

            println!("✅ Received PIR response in {:?}", start_tm.elapsed());

            println!("⏳ Processing PIR response...");
            let start_tm = Instant::now();

            match pir_client.process_response(query_key_as_bytes, &response) {
                Ok(response) => {
                    let json_obj: Map<String, Value> = serde_json::from_slice(&response).unwrap();
                    let json_str = serde_json::to_string_pretty(&json_obj).unwrap();

                    println!("✅ Decoded PIR response for query '{}': {}, in {:?}", query_key, json_str, start_tm.elapsed());
                }
                Err(e) => {
                    eprintln!("❌ Failed to process PIR response: {}", e);
                    return;
                }
            }

            println!("✅ Completing query took total time of {:?}", query_begins_at.elapsed());
        }
        Err(e) => {
            eprintln!("❌ Failed to prepare query: {}", e);
        }
    }
}

#[tokio::main]
async fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:7878").await.expect("❌ Unable to connect to server");
    println!("🎉 Connected to PIR server at 127.0.0.1:7878");

    request_setup_parameters(&mut stream).await;
    let setup_params_as_bytes = receive_setup_parameters(&mut stream).await.unwrap();
    let mut pir_client = setup_pir_client(setup_params_as_bytes);

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut input = String::new();

    loop {
        input.clear();
        println!("✍️ Type a query keyword and press Enter (or type 'quit' to exit):");

        match reader.read_line(&mut input).await {
            Ok(0) => {
                println!("❎ EOF reached, exiting.");
                break;
            }
            Ok(_) => {
                let trimmed = input.trim_end();
                let lowered = trimmed.to_lowercase();

                if lowered == "quit" {
                    println!("❎ Exiting.");
                    break;
                }

                handle_pir_query_for_key(&mut stream, &mut pir_client, trimmed).await;
            }
            Err(e) => {
                eprintln!("❌ Failed to read from stdin: {}", e);
                break;
            }
        }
    }
}
