//! Optional Radmin VPN assistant for Stick Studio.
//!
//! This helper does not bundle Radmin VPN and never disables the Windows
//! firewall. It detects the Radmin virtual adapter, reports its IPv4 address
//! and prepares the exact Direct Join address Stick already understands.

use std::{env, io::Write, net::Ipv4Addr, process::{Command, Stdio}, str::FromStr};

const STICK_PORT: u16 = 47_554;
const DOWNLOAD_URL: &str = "https://www.radmin-vpn.com/";

fn parse_radmin_ipv4(text: &str) -> Option<Ipv4Addr> {
    text.lines().map(str::trim).find_map(|line| {
        let ip=Ipv4Addr::from_str(line).ok()?;
        (ip.octets()[0]==26).then_some(ip)
    })
}

fn powershell_output(script: &str) -> Option<String> {
    let output=Command::new("powershell")
        .args(["-NoProfile","-NonInteractive","-Command",script])
        .output().ok()?;
    output.status.success().then(||String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn radmin_ipv4() -> Option<Ipv4Addr> {
    let script=r#"
Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
  Where-Object { $_.InterfaceAlias -match 'Radmin' -or $_.IPAddress -like '26.*' } |
  Select-Object -ExpandProperty IPAddress
"#;
    parse_radmin_ipv4(&powershell_output(script)?)
}

fn open_download_page() {
    let _=Command::new("cmd").args(["/C","start","",DOWNLOAD_URL]).status();
}

fn copy_join_address(address: &str) -> bool {
    let Ok(mut child)=Command::new("cmd")
        .args(["/C","clip"])
        .stdin(Stdio::piped()).spawn() else{return false};
    if let Some(mut stdin)=child.stdin.take() {
        if stdin.write_all(address.as_bytes()).is_err(){return false;}
    }
    child.wait().is_ok_and(|status|status.success())
}

fn ping(ip: Ipv4Addr) -> bool {
    Command::new("ping")
        .args(["-n","1","-w","1800",&ip.to_string()])
        .status().is_ok_and(|status|status.success())
}

fn status() {
    match radmin_ipv4() {
        Some(ip)=>{
            println!("Radmin VPN adapter: READY");
            println!("RADMIN_IP={ip}");
            println!("Stick host address: {ip}:{STICK_PORT}");
        }
        None=>{
            println!("Radmin VPN adapter was not found or has no 26.x IPv4 address.");
            println!("Install/start Radmin VPN, join/create a network, then run this helper again.");
            println!("Official download: {DOWNLOAD_URL}");
        }
    }
}

fn host() {
    println!("Stick + Radmin VPN — HOST");
    println!("1) In Radmin VPN choose Create network and set a network name/password.");
    println!("2) Give that Radmin network name/password only to people you trust.");
    println!("3) In Stick create the normal Online room and keep it open.");
    println!();
    match radmin_ipv4() {
        Some(ip)=>{
            let address=format!("{ip}:{STICK_PORT}");
            println!("READY");
            println!("Your Radmin IP: {ip}");
            println!("Your friend should use Stick -> Direct Join -> {address}");
            if copy_join_address(&address){println!("The Stick join address was copied to clipboard.");}
        }
        None=>{
            println!("Radmin VPN is not ready on this PC.");
            open_download_page();
        }
    }
}

fn join(raw: Option<&String>) {
    println!("Stick + Radmin VPN — GUEST");
    let Some(local)=radmin_ipv4() else{
        println!("Radmin VPN is not ready. Install/start it and join the host's Radmin network first.");
        open_download_page();
        return;
    };
    println!("Local Radmin IP: {local}");
    let Some(raw)=raw else{
        println!("Usage: StickRadminAssistant.exe join <host-radmin-ip>");
        println!("After joining the same Radmin network, enter the host's 26.x address.");
        return;
    };
    let Ok(host)=Ipv4Addr::from_str(raw.trim()) else{
        println!("Invalid IPv4 address: {raw}");
        return;
    };
    if host.octets()[0]!=26 {
        println!("That does not look like a Radmin VPN 26.x address.");
        return;
    }
    let address=format!("{host}:{STICK_PORT}");
    println!("Checking the host...");
    if ping(host) {
        println!("HOST REACHABLE");
    } else {
        println!("Ping did not answer. Radmin/firewall can block ping, so Direct Join may still work.");
    }
    println!("In Stick -> Direct Join enter: {address}");
    if copy_join_address(&address){println!("Copied: {address}");}
}

fn help() {
    println!("StickRadminAssistant — optional cross-network helper");
    println!();
    println!("Commands:");
    println!("  status");
    println!("  host");
    println!("  join <host-radmin-ip>");
    println!("  download");
    println!();
    println!("This helper does not install/bundle Radmin VPN and never disables your firewall.");
    println!("Both PCs must join the same Radmin VPN network.");
}

fn main() {
    let args:Vec<String>=env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("status")=>status(),
        Some("host")=>host(),
        Some("join")=>join(args.get(2)),
        Some("download")=>open_download_page(),
        _=>help(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_finds_only_radmin_26_range() {
        assert_eq!(parse_radmin_ipv4("192.168.1.4\n26.31.44.9\n10.0.0.1"),
            Some(Ipv4Addr::new(26,31,44,9)));
        assert_eq!(parse_radmin_ipv4("192.168.1.4\n10.0.0.1"),None);
    }

    #[test]
    fn stick_port_is_current_collaboration_port() {
        assert_eq!(STICK_PORT,47_554);
    }
}
