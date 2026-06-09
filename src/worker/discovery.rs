//! UDP broadcast discovery for MiniPWN workers.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

use crate::config::WorkerConfig;

pub const DISCOVERY_PORT: u16 = 10001;
const DISCOVERY_KIND: &str = "minipwn.discovery.v1";

#[derive(Debug, Serialize, Deserialize)]
struct DiscoveryRequest {
    kind: String,
    action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryAnnouncement {
    pub kind: String,
    pub action: String,
    pub name: String,
    pub port: u16,
    pub os: String,
    pub arch: String,
    pub secret_len: usize,
}

#[derive(Debug, Clone)]
pub struct DiscoveredWorker {
    pub name: String,
    pub url: String,
    pub os: String,
    pub arch: String,
    pub secret_len: usize,
}

/// Respond to LAN broadcast discovery requests.
pub async fn serve(config: WorkerConfig) -> Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)).await?;
    let mut buf = [0u8; 2048];

    loop {
        let (len, peer) = socket.recv_from(&mut buf).await?;
        let req = serde_json::from_slice::<DiscoveryRequest>(&buf[..len]);
        let Ok(req) = req else {
            continue;
        };
        if req.kind != DISCOVERY_KIND || req.action != "discover" {
            continue;
        }

        let announcement = DiscoveryAnnouncement {
            kind: DISCOVERY_KIND.to_string(),
            action: "announce".to_string(),
            name: format!("worker:{}", config.server.port),
            port: config.server.port,
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            secret_len: config.server.secret.len(),
        };
        let payload = serde_json::to_vec(&announcement)?;
        let _ = socket.send_to(&payload, peer).await;
    }
}

/// Broadcast a discovery request on known IPv4 broadcast addresses.
pub async fn discover(timeout: Duration) -> Result<Vec<DiscoveredWorker>> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;
    socket.set_broadcast(true)?;

    let req = DiscoveryRequest {
        kind: DISCOVERY_KIND.to_string(),
        action: "discover".to_string(),
    };
    let payload = serde_json::to_vec(&req)?;

    for target in broadcast_targets() {
        let _ = socket.send_to(&payload, target).await;
    }

    let started = Instant::now();
    let mut buf = [0u8; 2048];
    let mut seen = HashSet::new();
    let mut workers = Vec::new();

    while started.elapsed() < timeout {
        let remaining = timeout.saturating_sub(started.elapsed());
        let recv = tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await;
        let Ok(Ok((len, peer))) = recv else {
            break;
        };
        let Ok(announcement) = serde_json::from_slice::<DiscoveryAnnouncement>(&buf[..len]) else {
            continue;
        };
        if announcement.kind != DISCOVERY_KIND || announcement.action != "announce" {
            continue;
        }

        let url = format!("http://{}:{}", peer.ip(), announcement.port);
        if seen.insert(url.clone()) {
            workers.push(DiscoveredWorker {
                name: announcement.name,
                url,
                os: announcement.os,
                arch: announcement.arch,
                secret_len: announcement.secret_len,
            });
        }
    }

    Ok(workers)
}

fn broadcast_targets() -> Vec<SocketAddr> {
    let mut targets = HashSet::new();
    targets.insert(SocketAddr::from(([255, 255, 255, 255], DISCOVERY_PORT)));

    for ip in linux_interface_broadcasts() {
        targets.insert(SocketAddr::from((ip, DISCOVERY_PORT)));
    }

    targets.into_iter().collect()
}

fn linux_interface_broadcasts() -> Vec<Ipv4Addr> {
    let output = std::process::Command::new("ip")
        .args(["-o", "-4", "addr", "show"])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| parse_linux_broadcast(line))
        .collect()
}

fn parse_linux_broadcast(line: &str) -> Option<Ipv4Addr> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if let Some(pos) = tokens.iter().position(|t| *t == "brd") {
        return tokens.get(pos + 1)?.parse().ok();
    }

    let inet_pos = tokens.iter().position(|t| *t == "inet")?;
    let cidr = *tokens.get(inet_pos + 1)?;
    broadcast_from_cidr(cidr)
}

fn broadcast_from_cidr(cidr: &str) -> Option<Ipv4Addr> {
    let (ip, prefix) = cidr.split_once('/')?;
    let ip = ip.parse::<Ipv4Addr>().ok()?;
    let prefix = prefix.parse::<u32>().ok()?;
    if prefix > 32 {
        return None;
    }

    let ip_num = u32::from(ip);
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    Some(Ipv4Addr::from(ip_num | !mask))
}
