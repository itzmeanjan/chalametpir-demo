# chalametpir-demo
Demostration of using ChalametPIR - Keyword Private Information Retrieval, the Rust Library Crate @ https://github.com/itzmeanjan/ChalametPIR.

## Introduction
This binary crate demonstrates usage of Keyword Private Information Retrieval using the [ChalametPIR](https://crates.io/crates/chalamet_pir) library crate. It showcases a client-server architecture where the client can query a database of key-value pairs without revealing the queried keyword to the server.

## Prerequisites
Rust stable toolchain; see https://rustup.rs for installation guide. MSRV for this crate is 1.85.0.

```bash
# While developing this demonstration, I was using
$ rustc --version
rustc 1.85.0 (4d91de4e4 2025-02-17)
```

## Running the Demo
1. Run the PIR server.
```bash
cargo run --profile optimized --bin server db/crypto.json
```

2. In a separate terminal window, run the PIR client.
```bash
cargo run --profile optimized --bin client
```

3. **Query the Database:** The client will prompt you to enter keywords. Type in a keyword present in your database and hit Enter. The client will then display the corresponding value. Type 'quit' to exit the client.

> [!NOTE]
> For fetching the value associated with the queried keyword, client first prepares a PIR query using its local hint database, which it downloaded from the PIR server, when setting up the PIR client. Then it attempts to decode the received PIR response from the PIR server. If you prompt it to fetch value associated with some key which is absent in the database, the client will not be able to decode the response. During this execution of the protocol, server will never learn which keyword was queried by the client.

https://github.com/user-attachments/assets/5e5a1f43-bb34-409b-889a-54fede133153
