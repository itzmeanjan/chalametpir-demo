use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Handles an individual client connection.
async fn handle_client(mut stream: TcpStream) {
    println!("New connection from: {}", stream.peer_addr().unwrap());
    let mut buf = [0u8; 512];

    loop {
        match stream.read(&mut buf).await {
            Ok(0) => {
                // Connection closed
                println!("Connection closed");
                break;
            }
            Ok(n) => {
                // Echo the message back to the client
                let received = String::from_utf8_lossy(&buf[..n]);
                println!("Received: {}", received);
                if let Err(e) = stream.write_all(&buf[..n]).await {
                    eprintln!("Failed to write to client: {}", e);
                    break;
                }
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
    // Bind the TCP listener to an address.
    let listener = TcpListener::bind("127.0.0.1:7878")
        .await
        .expect("Failed to bind");

    println!("Server listening on 127.0.0.1:7878");

    // Accept incoming connections in an infinite loop.
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                // Spawn an asynchronous task for each client connection.
                tokio::spawn(async move {
                    handle_client(stream).await;
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}
