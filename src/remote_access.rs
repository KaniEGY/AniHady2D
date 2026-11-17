//! Optional Tailscale transport for friends on different Internet connections.
//! Stick remains a local TCP host; Tailscale supplies encrypted private routing
//! through NAT/CGNAT without opening a public TCP port.
//! Tailscale is separately installed and authenticated by the user; Stick does
//! not request credentials, modify firewall rules, or start a VPN service.
use serde_json::Value;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::{Command, Output};

#[derive(Debug, Clone, Default)]
pub struct TailnetInfo {
    pub address: Option<Ipv4Addr>,
    pub peers: Vec<Ipv4Addr>,
    pub connected: bool,
    pub available: bool,
}

fn tailscale_ipv4(address: &str) -> Option<Ipv4Addr> {
    let ip = address.parse::<Ipv4Addr>().ok()?;
    let octets = ip.octets();
    // Tailscale's IPv4 allocations are in the carrier-grade NAT 100.64/10 block.
    if octets[0] == 100 && (64..=127).contains(&octets[1]) { Some(ip) } else { None }
}

pub fn parse_status(data: &str) -> Result<TailnetInfo, String> {
    let payload: Value = serde_json::from_str(data).map_err(|e| e.to_string())?;
    let running = payload.get("BackendState").and_then(Value::as_str) == Some("Running");
    let mut info = TailnetInfo { available: true, connected: running, ..Default::default() };
    if !running { return Ok(info); }
    info.address = payload.pointer("/Self/TailscaleIPs")
        .and_then(Value::as_array)
        .and_then(|ips| ips.iter().filter_map(Value::as_str).find_map(tailscale_ipv4));
    if let Some(peers) = payload.get("Peer").and_then(Value::as_object) {
        for peer in peers.values() {
            if peer.get("Online").and_then(Value::as_bool) != Some(true) { continue; }
            if let Some(ips) = peer.get("TailscaleIPs").and_then(Value::as_array) {
                for ip in ips.iter().filter_map(Value::as_str).filter_map(tailscale_ipv4) {
                    if Some(ip) != info.address && !info.peers.contains(&ip) {
                        info.peers.push(ip);
                    }
                }
            }
        }
    }
    info.peers.sort_unstable();
    // Avoid issuing unbounded probes in larger corporate tailnets.
    info.peers.truncate(64);
    Ok(info)
}

fn status_output() -> Option<Output> {
    // Prefer the standard Windows signed install location when available.
    // Fall back to the installed CLI on PATH. Never interpolate user-supplied input.
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(root) = std::env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(root).join("Tailscale").join("tailscale.exe"));
    }
    candidates.push(PathBuf::from("tailscale"));
    for candidate in candidates {
        if let Ok(result) = Command::new(&candidate).args(["status", "--json"]).output() {
            return Some(result);
        }
    }
    None
}

pub fn current_tailnet() -> TailnetInfo {
    match status_output() {
        Some(output) if output.status.success() && output.stdout.len() <= 2_000_000 =>
            parse_status(&String::from_utf8_lossy(&output.stdout))
                .unwrap_or(TailnetInfo { available: true, ..Default::default() }),
        Some(_) => TailnetInfo { available: true, ..Default::default() },
        None => TailnetInfo::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_online_friends_and_ignores_offline_or_non_tailnet() {
        let input = r#"{
          "BackendState":"Running",
          "Self":{"TailscaleIPs":["100.78.14.22","fd7a:115c:a1e0::1"]},
          "Peer":{
            "friend": {"Online":true,"TailscaleIPs":["100.72.21.9"]},
            "sleeping":{"Online":false,"TailscaleIPs":["100.72.21.10"]},
            "other":{"Online":true,"TailscaleIPs":["192.168.1.5","100.72.21.9"]}
          }
        }"#;
        let result = parse_status(input).unwrap();
        assert!(result.connected);
        assert_eq!(result.address, Some("100.78.14.22".parse().unwrap()));
        assert_eq!(result.peers, vec!["100.72.21.9".parse::<Ipv4Addr>().unwrap()]);
    }
    #[test]
    fn not_logged_in_is_not_presented_as_a_working_remote_connection() {
        let result = parse_status(r#"{"BackendState":"NeedsLogin","Self":{"TailscaleIPs":["100.75.3.2"]}}"#).unwrap();
        assert!(result.available);
        assert!(!result.connected);
        assert!(result.address.is_none());
        assert!(result.peers.is_empty());
    }
    #[test]
    fn rejects_public_ips_not_owned_by_tailnet() {
        assert!(tailscale_ipv4("8.8.8.8").is_none());
        assert!(tailscale_ipv4("100.63.255.255").is_none());
        assert!(tailscale_ipv4("100.128.0.1").is_none());
        assert_eq!(tailscale_ipv4("100.64.0.1"), Some("100.64.0.1".parse().unwrap()));
    }
}
