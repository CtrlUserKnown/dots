//! A small point-in-time network monitor for the dashboard.
//!
//! [`probe`] gathers a snapshot of how well the network is working right now:
//! reachability + round-trip latency (measured as a TCP connect to a public
//! resolver on port 53), the active network's name (Wi-Fi SSID or connection
//! name), and the DNS server(s) currently in use.
//!
//! Everything is best-effort: any piece we can't determine comes back as `None`
//! / empty rather than failing, so the monitor degrades gracefully on hosts
//! where the helper commands aren't available. Probing runs on a background
//! thread (see the TUI event loop) because a TCP connect and a couple of small
//! subprocess calls are far too slow to run inline on every frame.

use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// Hosts we try, in order, to measure reachability and latency. Port 53 (DNS)
/// is almost never firewalled and these are highly-available anycast resolvers.
const PROBE_HOSTS: &[&str] = &["1.1.1.1:53", "8.8.8.8:53"];
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// A snapshot of network state at a single point in time.
#[derive(Debug, Clone, Default)]
pub struct NetworkStatus {
    /// Whether we could reach the internet at all.
    pub online: bool,
    /// Round-trip latency of the successful reachability probe, in milliseconds.
    pub latency_ms: Option<u32>,
    /// Active network name — Wi-Fi SSID or NetworkManager connection name.
    pub name: Option<String>,
    /// DNS server(s) currently configured.
    pub dns: Vec<String>,
    /// Whether a VPN tunnel appears to be active.
    pub vpn: bool,
}

impl NetworkStatus {
    /// A short qualitative label for the current latency ("excellent"…"slow").
    pub fn quality(&self) -> Option<&'static str> {
        self.latency_ms.map(|ms| match ms {
            0..=49 => "excellent",
            50..=149 => "good",
            150..=399 => "fair",
            _ => "slow",
        })
    }
}

/// Take a fresh network snapshot. Never fails — unknown fields are left empty.
pub fn probe() -> NetworkStatus {
    let (online, latency_ms) = measure_reachability();
    NetworkStatus {
        online,
        latency_ms,
        name: network_name(),
        dns: dns_servers(),
        vpn: vpn_active(),
    }
}

/// Try each probe host in turn, returning `(reachable, latency)` for the first
/// that connects within the timeout.
fn measure_reachability() -> (bool, Option<u32>) {
    for host in PROBE_HOSTS {
        let Ok(addr) = host.parse::<SocketAddr>() else { continue };
        let start = Instant::now();
        if TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).is_ok() {
            let ms = start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
            return (true, Some(ms));
        }
    }
    (false, None)
}

/// Run a helper command and return its stdout as a trimmed `String`, or `None`
/// if the binary is missing or exits non-zero.
fn command_output(cmd: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// ── platform-specific: active network name ──────────────────────────────────

#[cfg(target_os = "macos")]
fn network_name() -> Option<String> {
    // Ask each Wi-Fi-capable interface for its current SSID.
    for iface in ["en0", "en1"] {
        if let Some(out) = command_output("networksetup", &["-getairportnetwork", iface]) {
            // "Current Wi-Fi Network: <SSID>"  |  "You are not associated…"
            if let Some((_, ssid)) = out.split_once(": ") {
                let ssid = ssid.trim();
                if !ssid.is_empty() {
                    return Some(ssid.to_string());
                }
            }
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn network_name() -> Option<String> {
    // Prefer the active Wi-Fi SSID, then fall back to the active connection name.
    if let Some(out) = command_output("nmcli", &["-t", "-f", "ACTIVE,SSID", "dev", "wifi"]) {
        for line in out.lines() {
            if let Some(ssid) = line.strip_prefix("yes:") {
                let ssid = ssid.trim();
                if !ssid.is_empty() {
                    return Some(ssid.to_string());
                }
            }
        }
    }
    if let Some(out) = command_output("nmcli", &["-t", "-f", "NAME", "connection", "show", "--active"]) {
        if let Some(name) = out.lines().find(|l| !l.trim().is_empty()) {
            return Some(name.trim().to_string());
        }
    }
    None
}

// ── platform-specific: DNS servers ──────────────────────────────────────────

#[cfg(target_os = "macos")]
fn dns_servers() -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    if let Some(out) = command_output("scutil", &["--dns"]) {
        for line in out.lines() {
            let line = line.trim();
            // Lines look like: "nameserver[0] : 192.168.1.1"
            if line.starts_with("nameserver[") {
                if let Some((_, ip)) = line.split_once(':') {
                    let ip = ip.trim().to_string();
                    if !ip.is_empty() && !seen.contains(&ip) {
                        seen.push(ip);
                    }
                }
            }
        }
    }
    seen.truncate(3);
    seen
}

#[cfg(not(target_os = "macos"))]
fn dns_servers() -> Vec<String> {
    let from_resolv = resolv_conf_nameservers();

    // systemd-resolved installs a loopback stub (127.0.0.53) in resolv.conf that
    // hides the real upstream servers — ask resolvectl for those instead.
    let only_stub = !from_resolv.is_empty() && from_resolv.iter().all(|s| s.starts_with("127."));
    if from_resolv.is_empty() || only_stub {
        if let Some(real) = resolvectl_dns() {
            if !real.is_empty() {
                return real;
            }
        }
    }
    from_resolv
}

#[cfg(not(target_os = "macos"))]
fn resolv_conf_nameservers() -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    if let Ok(text) = std::fs::read_to_string("/etc/resolv.conf") {
        for line in text.lines() {
            if let Some(rest) = line.trim().strip_prefix("nameserver") {
                let ip = rest.trim().to_string();
                if !ip.is_empty() && !seen.contains(&ip) {
                    seen.push(ip);
                }
            }
        }
    }
    seen.truncate(3);
    seen
}

/// Real upstream DNS behind the systemd-resolved stub, from `resolvectl status`.
#[cfg(not(target_os = "macos"))]
fn resolvectl_dns() -> Option<Vec<String>> {
    let out = command_output("resolvectl", &["status"])?;
    // Prefer the first non-empty "DNS Servers:" list (the active upstream).
    for line in out.lines() {
        if let Some(rest) = line.trim().strip_prefix("DNS Servers:") {
            let servers: Vec<String> =
                rest.split_whitespace().map(str::to_string).take(3).collect();
            if !servers.is_empty() {
                return Some(servers);
            }
        }
    }
    // Fall back to the single "Current DNS Server:" line.
    for line in out.lines() {
        if let Some(rest) = line.trim().strip_prefix("Current DNS Server:") {
            let s = rest.trim();
            if !s.is_empty() {
                return Some(vec![s.to_string()]);
            }
        }
    }
    None
}

// ── platform-specific: VPN detection ────────────────────────────────────────

#[cfg(target_os = "macos")]
fn vpn_active() -> bool {
    // Built-in and app-registered VPN services (IKEv2/IPsec/L2TP, and many
    // third-party clients that register a network service).
    if let Some(out) = command_output("scutil", &["--nc", "list"]) {
        if out.lines().any(|l| l.contains("(Connected)")) {
            return true;
        }
    }
    // WireGuard/OpenVPN-style clients instead expose a `utun` interface carrying
    // an IPv4 address — the always-on system utuns generally don't have one.
    if let Some(out) = command_output("ifconfig", &[]) {
        let mut in_utun = false;
        for line in out.lines() {
            if !line.starts_with(char::is_whitespace) {
                in_utun = line.starts_with("utun");
            } else if in_utun && line.trim_start().starts_with("inet ") {
                return true;
            }
        }
    }
    false
}

#[cfg(not(target_os = "macos"))]
fn vpn_active() -> bool {
    // A VPN client creates a virtual interface. TUN/TAP tunnels (OpenVPN,
    // Tailscale, …) expose a `tun_flags` file regardless of their name, which is
    // the most reliable signal; WireGuard uses its own device type, matched by
    // the conventional `wg*`/`ppp*`/`ipsec*` naming. `operstate` is often
    // "unknown" for the point-to-point links VPNs use, so accept that too.
    let Ok(entries) = std::fs::read_dir("/sys/class/net") else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_tunnel = path.join("tun_flags").exists()
            || name.starts_with("wg")
            || name.starts_with("ppp")
            || name.starts_with("ipsec");
        if is_tunnel {
            let state = std::fs::read_to_string(path.join("operstate")).unwrap_or_default();
            if matches!(state.trim(), "up" | "unknown") {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_buckets_map_correctly() {
        let q = |ms| NetworkStatus { latency_ms: Some(ms), ..Default::default() }.quality();
        assert_eq!(q(10), Some("excellent"));
        assert_eq!(q(80), Some("good"));
        assert_eq!(q(200), Some("fair"));
        assert_eq!(q(900), Some("slow"));
        assert_eq!(NetworkStatus::default().quality(), None);
    }

    #[test]
    fn probe_never_panics() {
        // We can't assert connectivity in CI, but probing must always return.
        let _ = probe();
    }
}
