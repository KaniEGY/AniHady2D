//! Optional remote-network bridge for Stick Studio.
//!
//! The bridge tunnels Stick's existing localhost TCP collaboration protocol over
//! authenticated Iroh QUIC. Iroh tries direct NAT traversal first and can fall
//! back to its configured relay infrastructure. Stick's room password challenge
//! and protocol validation remain unchanged because the TCP bytes are forwarded
//! transparently.
//!
//! Host:
//!   StickOnlineBridge.exe host
//! Guest:
//!   StickOnlineBridge.exe join stick-remote://<HOST_ENDPOINT_ID>
//! Then the guest enters 127.0.0.1:47555 in Stick's existing Direct Join box.

use anyhow::{bail, Context, Result};
use iroh::{
    Endpoint, EndpointId,
    endpoint::{presets, RecvStream, SendStream},
};
use std::{env, str::FromStr};
use tokio::{
    io::{AsyncWriteExt, copy},
    net::{TcpListener, TcpStream},
};

const ALPN: &[u8] = b"stick-online-bridge/1";
const DEFAULT_STICK_PORT: u16 = 47_554;
const DEFAULT_GUEST_PORT: u16 = 47_555;
const INVITE_PREFIX: &str = "stick-remote://";

fn parse_port(value: Option<&String>, default: u16) -> Result<u16> {
    match value {
        Some(raw) => {
            let port: u16 = raw.parse().with_context(|| format!("Invalid TCP port: {raw}"))?;
            if port == 0 { bail!("TCP port cannot be zero"); }
            Ok(port)
        }
        None => Ok(default),
    }
}

fn parse_remote_code(value: &str) -> Result<EndpointId> {
    let raw = value.trim().strip_prefix(INVITE_PREFIX).unwrap_or(value.trim());
    EndpointId::from_str(raw).with_context(|| "Invalid Stick remote code")
}

async fn bridge_streams(
    tcp: TcpStream,
    mut remote_send: SendStream,
    mut remote_recv: RecvStream,
) -> Result<()> {
    let (mut tcp_read, mut tcp_write) = tcp.into_split();

    let upload = async {
        let bytes = copy(&mut tcp_read, &mut remote_send)
            .await
            .context("Local Stick -> remote bridge stream failed")?;
        remote_send.finish().context("Failed to finish remote send stream")?;
        Ok::<u64, anyhow::Error>(bytes)
    };

    let download = async {
        let bytes = copy(&mut remote_recv, &mut tcp_write)
            .await
            .context("Remote bridge -> local Stick stream failed")?;
        tcp_write.shutdown().await.context("Failed to close local Stick socket")?;
        Ok::<u64, anyhow::Error>(bytes)
    };

    let _ = tokio::try_join!(upload, download)?;
    Ok(())
}

async fn run_host(stick_port: u16) -> Result<()> {
    let endpoint = Endpoint::builder(presets::N0)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .context("Could not start the encrypted remote endpoint")?;

    println!("Stick Online Bridge — HOST");
    println!("Connecting to the remote discovery/relay network...");
    endpoint.online().await;

    let endpoint_id = endpoint.id();
    println!();
    println!("READY");
    println!("STICK_REMOTE_CODE={INVITE_PREFIX}{endpoint_id}");
    println!("Share that code with the guest. Do NOT share the Stick room password.");
    println!("Local Stick host must stay open on 127.0.0.1:{stick_port}.");
    println!("Press Ctrl+C to stop the bridge.");
    println!();

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Stopping remote bridge...");
                endpoint.close().await;
                return Ok(());
            }
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else {
                    bail!("Remote endpoint closed unexpectedly");
                };
                tokio::spawn(async move {
                    let result: Result<()> = async {
                        let connection = incoming.await
                            .context("Remote peer handshake failed")?;
                        let remote = connection.remote_id();
                        let (send, recv) = connection.accept_bi().await
                            .context("Remote peer did not open a bridge stream")?;
                        let tcp = TcpStream::connect(("127.0.0.1", stick_port)).await
                            .with_context(|| format!(
                                "Cannot reach local Stick host on 127.0.0.1:{stick_port}"
                            ))?;
                        println!("Remote peer {remote} connected.");
                        bridge_streams(tcp, send, recv).await?;
                        println!("Remote peer {remote} disconnected.");
                        Ok(())
                    }.await;
                    if let Err(error) = result {
                        eprintln!("Remote session error: {error:#}");
                    }
                });
            }
        }
    }
}

async fn run_guest(remote: EndpointId, local_port: u16) -> Result<()> {
    let endpoint = Endpoint::bind(presets::N0)
        .await
        .context("Could not start the encrypted remote endpoint")?;

    println!("Stick Online Bridge — GUEST");
    println!("Connecting to the remote discovery/relay network...");
    endpoint.online().await;

    let listener = TcpListener::bind(("127.0.0.1", local_port)).await
        .with_context(|| format!(
            "Cannot open local bridge port 127.0.0.1:{local_port}. Is another bridge already running?"
        ))?;

    println!();
    println!("READY");
    println!("In Stick -> Join directly, enter: 127.0.0.1:{local_port}");
    println!("Keep this add-on open while using the room.");
    println!("Press Ctrl+C to stop the bridge.");
    println!();

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Stopping remote bridge...");
                endpoint.close().await;
                return Ok(());
            }
            accepted = listener.accept() => {
                let (tcp, local_addr) = accepted.context("Local Stick connection failed")?;
                let endpoint = endpoint.clone();
                tokio::spawn(async move {
                    let result: Result<()> = async {
                        let connection = endpoint.connect(remote, ALPN).await
                            .context("Could not reach the remote Stick host")?;
                        let (send, recv) = connection.open_bi().await
                            .context("Could not open encrypted bridge stream")?;
                        // A newly opened QUIC stream is not visible to the receiver
                        // until data is written; Stick's TCP handshake sends first.
                        println!("Local Stick {local_addr} connected to remote host.");
                        bridge_streams(tcp, send, recv).await?;
                        connection.close(0u32.into(), b"Stick bridge stream closed");
                        Ok(())
                    }.await;
                    if let Err(error) = result {
                        eprintln!("Bridge session error: {error:#}");
                    }
                });
            }
        }
    }
}

fn print_help() {
    println!("StickOnlineBridge — optional cross-network add-on");
    println!();
    println!("HOST:");
    println!("  StickOnlineBridge.exe host [stick_tcp_port]");
    println!("  Default Stick host port: {DEFAULT_STICK_PORT}");
    println!();
    println!("GUEST:");
    println!("  StickOnlineBridge.exe join <stick-remote://HOST_ID> [local_tcp_port]");
    println!("  Default local guest bridge: 127.0.0.1:{DEFAULT_GUEST_PORT}");
    println!();
    println!("The add-on only listens on localhost. Stick's existing room password");
    println!("and protocol checks still protect the collaboration room.");
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("host") => {
            let port = parse_port(args.get(2), DEFAULT_STICK_PORT)?;
            run_host(port).await
        }
        Some("join") => {
            let code = args.get(2).context(
                "Missing remote code. Usage: StickOnlineBridge.exe join stick-remote://HOST_ID"
            )?;
            let remote = parse_remote_code(code)?;
            let port = parse_port(args.get(3), DEFAULT_GUEST_PORT)?;
            run_guest(remote, port).await
        }
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some(other) => bail!("Unknown command: {other}. Use --help."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_use_safe_defaults_and_reject_zero() {
        assert_eq!(parse_port(None, DEFAULT_STICK_PORT).unwrap(), DEFAULT_STICK_PORT);
        let zero = "0".to_string();
        assert!(parse_port(Some(&zero), DEFAULT_STICK_PORT).is_err());
    }

    #[test]
    fn remote_code_parser_accepts_prefixed_endpoint_id() {
        // EndpointId uses the same textual representation as its public key.
        let secret = iroh::SecretKey::generate(&mut rand::rngs::OsRng);
        let id = secret.public();
        let code = format!("{INVITE_PREFIX}{id}");
        assert_eq!(parse_remote_code(&code).unwrap(), id);
    }
}
