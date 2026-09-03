use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use anyhow::Result;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::task::JoinSet;
use tokio::time::Instant;

use super::types::DlnaSsdpDevice;

const SSDP_ADDR_V4: &str = "239.255.255.250:1900";

pub(super) async fn ssdp_msearch_multi_iface(
    st: &str,
    mx: u8,
    timeout: Duration,
) -> Result<Vec<DlnaSsdpDevice>> {
    let ips = candidate_ipv4_addrs();

    // Fallback: bind to 0.0.0.0 if we can't enumerate interfaces for some reason.
    if ips.is_empty() {
        return ssdp_msearch_on_socket(
            UdpSocket::bind(("0.0.0.0", 0)).await?,
            None,
            st.to_string(),
            mx,
            timeout,
        )
        .await;
    }

    let mut join = JoinSet::new();
    for ip in ips {
        let st = st.to_string();
        join.spawn(async move {
            let socket = bind_udp_on_iface(ip)?;
            ssdp_msearch_on_socket(socket, Some(ip), st, mx, timeout).await
        });
    }

    let mut devices = Vec::new();
    let mut seen_usn: HashSet<String> = HashSet::new();
    while let Some(res) = join.join_next().await {
        match res {
            Ok(Ok(list)) => {
                for d in list {
                    if seen_usn.insert(d.usn.clone()) {
                        devices.push(d);
                    }
                }
            },
            Ok(Err(e)) => tracing::debug!("ssdp m-search iface task failed: {e:#}"),
            Err(e) => tracing::debug!("ssdp m-search iface join failed: {e}"),
        }
    }

    Ok(devices)
}

async fn ssdp_msearch_on_socket(
    socket: UdpSocket,
    local_ip: Option<Ipv4Addr>,
    st: String,
    mx: u8,
    timeout: Duration,
) -> Result<Vec<DlnaSsdpDevice>> {
    socket.set_broadcast(true)?;
    // Some platforms return an error for multicast TTL; it's fine to ignore.
    let _ = socket.set_multicast_ttl_v4(2);

    let req = format!(
        "M-SEARCH * HTTP/1.1\r\n\
HOST: {SSDP_ADDR_V4}\r\n\
MAN: \"ssdp:discover\"\r\n\
MX: {mx}\r\n\
ST: {st}\r\n\
\r\n"
    );
    tracing::debug!(
        "ssdp m-search st={st} mx={mx} timeout_ms={} local_ip={:?}",
        timeout.as_millis(),
        local_ip
    );
    socket.send_to(req.as_bytes(), SSDP_ADDR_V4).await?;

    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 8192];
    let mut devices = Vec::new();
    let mut seen_usn: HashSet<String> = HashSet::new();

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline - now;
        let remaining = remaining.min(Duration::from_millis(250));

        let recv = tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await;
        match recv {
            Ok(Ok((len, from))) => {
                if let Some(d) = parse_ssdp_response(&buf[..len]) {
                    if !d.st.eq_ignore_ascii_case(&st) {
                        continue;
                    }
                    if seen_usn.insert(d.usn.clone()) {
                        tracing::debug!(
                            "ssdp response from={} usn={} st={} location={} local_ip={:?}",
                            from,
                            d.usn,
                            d.st,
                            d.location,
                            local_ip
                        );
                        devices.push(d);
                    }
                }
            },
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => continue,
        }
    }

    Ok(devices)
}

fn bind_udp_on_iface(local_ip: Ipv4Addr) -> Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_broadcast(true)?;
    // Some platforms return an error for multicast TTL/iface; it's fine to ignore.
    let _ = socket.set_multicast_ttl_v4(2);
    let _ = socket.set_multicast_if_v4(&local_ip);
    socket.bind(&socket2::SockAddr::from(std::net::SocketAddrV4::new(
        local_ip, 0,
    )))?;
    socket.set_nonblocking(true)?;
    let socket: std::net::UdpSocket = socket.into();
    Ok(UdpSocket::from_std(socket)?)
}

pub(super) fn candidate_ipv4_addrs() -> Vec<Ipv4Addr> {
    let mut out: Vec<Ipv4Addr> = Vec::new();

    let addrs = match get_if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(_) => return out,
    };

    for iface in addrs {
        let name = iface.name.to_ascii_lowercase();
        // Heuristic: skip common VPN/tunnel adapters so we prefer LAN interfaces for SSDP + HTTP.
        // (When a VPN is enabled on Windows, the default route may point to a tunnel interface.)
        if name.contains("wintun")
            || name.contains("wireguard")
            || name.contains("tailscale")
            || name.contains("zerotier")
            || name.contains("openvpn")
            || name.contains("vpn")
            || name.contains("tap")
            || name.contains("tun")
        {
            continue;
        }
        // Also skip common virtual adapters that often produce private IPs not reachable from LAN
        // devices (WSL/Hyper-V/Docker/VMware/VirtualBox).
        if name.contains("vethernet")
            || name.contains("hyper-v")
            || name.contains("wsl")
            || name.contains("docker")
            || name.contains("vmware")
            || name.contains("virtualbox")
            || name.contains("loopback")
        {
            continue;
        }

        let ip = match iface.ip() {
            IpAddr::V4(v) => v,
            IpAddr::V6(_) => continue,
        };
        if ip.is_loopback() {
            continue;
        }
        // Link-local 169.254.0.0/16
        if ip.octets()[0] == 169 && ip.octets()[1] == 254 {
            continue;
        }
        out.push(ip);
    }

    // Prefer private RFC1918 addresses. If none, return whatever we found.
    let mut private = out
        .iter()
        .copied()
        .filter(|ip| is_private_rfc1918(*ip))
        .collect::<Vec<_>>();
    if !private.is_empty() {
        // Rank common home LAN ranges ahead of other private ranges.
        private.sort_by_key(|ip| (private_ipv4_rank(*ip), *ip));
        private.dedup();
        return private;
    }

    out.sort();
    out.dedup();
    out
}

fn private_ipv4_rank(ip: Ipv4Addr) -> u8 {
    let [a, b, _, _] = ip.octets();
    // Most home routers use 192.168.0.0/16. Many VPN/tunnels also use 10.0.0.0/8.
    // We prefer 192.168 first, then 10, then 172.16/12.
    if a == 192 && b == 168 {
        0
    } else if a == 10 {
        1
    } else if a == 172 && (16..=31).contains(&b) {
        2
    } else {
        3
    }
}

fn is_private_rfc1918(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168)
}

fn parse_ssdp_response(bytes: &[u8]) -> Option<DlnaSsdpDevice> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.split("\r\n");
    let status = lines.next()?.trim();
    if !status.starts_with("HTTP/1.1 200") {
        return None;
    }

    let mut usn: Option<String> = None;
    let mut st: Option<String> = None;
    let mut location: Option<String> = None;
    let mut server: Option<String> = None;

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let key = k.trim().to_ascii_lowercase();
        let value = v.trim().to_string();
        match key.as_str() {
            "usn" => usn = Some(value),
            "st" => st = Some(value),
            "location" => location = Some(value),
            "server" => server = Some(value),
            _ => {},
        }
    }

    Some(DlnaSsdpDevice {
        usn: usn?,
        st: st?,
        location: location?,
        server,
    })
}
