use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::process::Command as AsyncCommand;
use tokio::time::timeout;

const CMD_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceScope {
    System,
    User,
}

#[derive(Debug, Clone)]
pub struct Service {
    pub name: String,
    pub enabled: bool,
    pub active: bool,
}

pub fn list_services(scope: &ServiceScope) -> Result<Vec<Service>> {
    // Get unit-file states (enabled/disabled)
    let mut cmd = Command::new("systemctl");
    if *scope == ServiceScope::User {
        cmd.arg("--user");
    }
    cmd.args([
        "list-unit-files",
        "--type=service",
        "--no-pager",
        "--no-legend",
    ]);

    let output = cmd.output().context("Failed to run systemctl")?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Get active/running states
    let active_set = get_active_services(scope);

    let services = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[0].to_string();
                let state = parts[1];
                // Only include services that can be manually enabled/disabled.
                // Skip static, generated, alias, transient, indirect, masked.
                let toggleable = matches!(
                    state,
                    "enabled" | "enabled-runtime" | "disabled" | "linked" | "linked-runtime"
                );
                if !toggleable {
                    return None;
                }
                let enabled = matches!(state, "enabled" | "enabled-runtime" | "linked");
                let active = active_set.contains(&name);
                Some(Service {
                    name,
                    enabled,
                    active,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(services)
}

fn get_active_services(scope: &ServiceScope) -> std::collections::HashSet<String> {
    let mut cmd = Command::new("systemctl");
    if *scope == ServiceScope::User {
        cmd.arg("--user");
    }
    cmd.args([
        "list-units",
        "--type=service",
        "--state=active",
        "--no-pager",
        "--no-legend",
    ]);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return std::collections::HashSet::new(),
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().next().map(|s| s.to_string()))
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct ServiceInfo {
    pub description: String,
    pub active_state: String,
    pub sub_state: String,
    pub fragment_path: String,
    pub triggered_by: String,
    pub documentation: String,
    pub extra_info: String,
}

pub fn get_service_info(scope: &ServiceScope, service: &str) -> ServiceInfo {
    let is_template = service.contains('@');

    // For template units, try instantiated form or fall back to systemctl cat
    let mut info = if is_template {
        get_info_from_cat(scope, service)
    } else {
        get_info_from_show(scope, service)
    };

    // Enrich with curated descriptions when systemd's own description is generic
    if let Some(extra) = curated_description(service) {
        info.extra_info = extra.to_string();
    }

    info
}

fn get_info_from_show(scope: &ServiceScope, service: &str) -> ServiceInfo {
    let mut cmd = Command::new("systemctl");
    if *scope == ServiceScope::User {
        cmd.arg("--user");
    }
    cmd.args([
        "show",
        service,
        "-p",
        "Description,ActiveState,SubState,FragmentPath,TriggeredBy,Documentation",
        "--no-pager",
    ]);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return ServiceInfo::default(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut info = ServiceInfo::default();

    for line in stdout.lines() {
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "Description" => info.description = value.to_string(),
                "ActiveState" => info.active_state = value.to_string(),
                "SubState" => info.sub_state = value.to_string(),
                "FragmentPath" => info.fragment_path = value.to_string(),
                "TriggeredBy" => info.triggered_by = value.to_string(),
                "Documentation" => info.documentation = value.to_string(),
                _ => {}
            }
        }
    }

    info
}

fn get_info_from_cat(scope: &ServiceScope, service: &str) -> ServiceInfo {
    let mut cmd = Command::new("systemctl");
    if *scope == ServiceScope::User {
        cmd.arg("--user");
    }
    cmd.args(["cat", service, "--no-pager"]);

    let output = match cmd.output() {
        Ok(o) if o.status.success() => o,
        _ => return ServiceInfo::default(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut info = ServiceInfo::default();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("Description=") {
            info.description = val.to_string();
        } else if let Some(val) = trimmed.strip_prefix("Documentation=") {
            info.documentation = val.to_string();
        } else if trimmed.starts_with("# /") {
            info.fragment_path = trimmed.trim_start_matches("# ").to_string();
        }
    }

    // Template units aren't running instances, so state isn't meaningful
    info.active_state = "template".to_string();
    info.sub_state = "n/a".to_string();

    info
}

fn curated_description(service: &str) -> Option<&'static str> {
    let name = service.trim_end_matches(".service");
    // Strip template suffix for matching (e.g., "ly@" -> "ly")
    let base = name.split('@').next().unwrap_or(name);

    match base {
        // Display managers
        "gdm" => Some("GNOME Display Manager. Provides graphical login screen and manages user sessions. Handles X11/Wayland session startup."),
        "sddm" => Some("Simple Desktop Display Manager. Qt-based login screen, commonly used with KDE Plasma."),
        "lightdm" => Some("Lightweight Display Manager. Cross-desktop login screen supporting multiple greeters."),
        "ly" => Some("Lightweight TUI display manager. Provides a terminal-based login screen as an alternative to graphical display managers."),
        "greetd" => Some("Minimal login daemon. Supports pluggable greeter frontends (tuigreet, gtkgreet, etc.)."),

        // Network
        "NetworkManager" => Some("Desktop network management daemon. Manages WiFi, Ethernet, VPN, and mobile broadband connections. Provides nm-applet tray icon."),
        "NetworkManager-dispatcher" => Some("Runs scripts in response to network events (connect/disconnect). Scripts live in /etc/NetworkManager/dispatcher.d/."),
        "NetworkManager-wait-online" => Some("Blocks boot until network is fully connected. Needed by services requiring network at startup. Can slow boot if network is slow."),
        "systemd-networkd" => Some("Systemd's built-in network manager. Lighter alternative to NetworkManager, configured via .network files in /etc/systemd/network/."),
        "systemd-resolved" => Some("Systemd DNS resolver. Provides DNS caching, DNSSEC validation, and DNS-over-TLS. Manages /etc/resolv.conf."),
        "wpa_supplicant" => Some("WiFi authentication daemon (WPA/WPA2/WPA3). Usually managed by NetworkManager, but can run standalone for simpler setups."),
        "iwd" => Some("Intel Wireless Daemon. Modern alternative to wpa_supplicant with simpler config. Can be used as NetworkManager's WiFi backend."),

        // Audio
        "pipewire" => Some("Modern audio/video server replacing PulseAudio and JACK. Handles screen sharing, Bluetooth audio, and low-latency audio."),
        "wireplumber" => Some("Session manager for PipeWire. Handles audio routing policy, device management, and Bluetooth audio profiles."),
        "pulseaudio" => Some("Legacy audio server. Being replaced by PipeWire on most modern Linux desktops."),

        // Bluetooth
        "bluetooth" => Some("BlueZ Bluetooth daemon. Manages Bluetooth device pairing, connections, and profiles (A2DP, HFP, etc.)."),
        "blueman-mechanism" => Some("Blueman privilege helper. Allows the Blueman Bluetooth manager applet to perform system-level Bluetooth operations."),

        // Printing
        "cups" => Some("Common Unix Printing System. Manages print queues, printer discovery (via Avahi/mDNS), and IPP printing. Web UI at localhost:631."),
        "avahi-daemon" => Some("mDNS/DNS-SD daemon for zero-configuration networking. Enables .local hostname resolution and network service discovery (printers, etc.)."),
        "avahi-dnsconfd" => Some("Configures DNS servers discovered via Avahi. Rarely needed if using NetworkManager or systemd-resolved."),

        // Security / Firewall
        "sshd" => Some("OpenSSH server daemon. Accepts incoming SSH connections for remote shell access, file transfer (scp/sftp), and tunneling."),
        "ufw" => Some("Uncomplicated Firewall. User-friendly frontend for iptables/nftables. Manages incoming/outgoing traffic rules."),
        "firewalld" => Some("Dynamic firewall daemon with zones. Uses nftables backend. Supports runtime changes without restarting."),
        "nftables" => Some("Netfilter tables. Modern kernel packet filtering framework replacing iptables. Rules in /etc/nftables.conf."),
        "apparmor" => Some("Mandatory Access Control security framework. Confines programs to limited resources using per-program profiles."),
        "auditd" => Some("Linux Audit daemon. Logs security-relevant events (file access, syscalls, authentication) per configured rules."),
        "fail2ban" => Some("Intrusion prevention. Monitors log files and bans IPs showing malicious signs (brute-force SSH, etc.) via firewall rules."),

        // Power / Hardware
        "upower" => Some("Power management abstraction. Provides battery info, suspend/hibernate support. Used by desktop environments for power status."),
        "power-profiles-daemon" => Some("Provides power profile switching (balanced, power-saver, performance). Used by GNOME/KDE power settings."),
        "cpupower" => Some("CPU frequency scaling. Sets CPU governor (performance/powersave/schedutil) at boot. Config in /etc/default/cpupower."),
        "lm_sensors" => Some("Hardware monitoring. Reads CPU/GPU temperatures, fan speeds, and voltages from sensor chips."),
        "smartd" => Some("S.M.A.R.T. disk monitoring daemon. Watches hard drive health indicators and warns of impending failures."),
        "fancontrol" => Some("Fan speed control daemon. Uses lm_sensors data to dynamically adjust fan speeds based on temperature."),

        // Containers
        "docker" => Some("Docker container runtime. Manages container images, networks, and volumes. API on /var/run/docker.sock."),
        "podman" => Some("Daemonless container engine. Docker-compatible CLI but runs rootless by default. No persistent daemon needed."),
        "containerd" => Some("Container runtime daemon. Low-level container execution used by Docker and Kubernetes."),

        // Systemd core
        "systemd-timesyncd" => Some("Simple NTP client. Synchronizes system clock with network time servers. Lighter alternative to chrony/ntpd."),
        "systemd-oomd" => Some("Out-of-memory daemon. Monitors memory pressure and kills cgroup trees before the kernel OOM killer triggers."),
        "systemd-homed" => Some("Portable home directory manager. Stores home dirs as LUKS-encrypted images that can move between machines."),
        "systemd-boot-update" => Some("Automatically updates systemd-boot EFI bootloader when systemd is upgraded."),
        "systemd-pstore" => Some("Persistent storage for kernel crash dumps. Copies pstore data (dmesg, etc.) from /sys/fs/pstore to /var/lib/systemd/pstore."),

        // Misc system services
        "accounts-daemon" => Some("D-Bus service for user account management. Used by GDM and GNOME Settings for user info, avatar, and language preferences."),
        "rtkit-daemon" => Some("RealtimeKit. Safely grants realtime scheduling priority to user processes (PipeWire, audio apps) without running them as root."),
        "udisks2" => Some("Disk management daemon. Provides D-Bus API for mounting/unmounting drives, used by file managers for removable media."),
        "ModemManager" => Some("Mobile broadband modem management. Controls 3G/4G/5G modems and provides connection setup. Safe to disable without mobile broadband."),
        "haveged" => Some("Entropy harvesting daemon. Feeds additional randomness to /dev/random. Less needed on modern kernels with good entropy sources."),
        "gpm" => Some("General Purpose Mouse. Provides mouse support in Linux virtual consoles (TTY). Not needed in graphical environments."),
        "reflector" => Some("Arch Linux mirrorlist updater. Fetches latest mirror list and sorts by speed/country. Usually run via timer, not continuously."),

        // Arch / CachyOS specific
        "ananicy-cpp" => Some("Auto Nice Daemon (C++ rewrite). Automatically adjusts process priorities and I/O scheduling for better desktop responsiveness."),
        "cachyos-rate-mirrors" => Some("CachyOS mirror rating. Tests and sorts pacman mirrors by speed for faster package downloads."),
        "scx_loader" => Some("Sched-ext loader. Loads custom Linux CPU schedulers (BORE, Rusty, etc.) for CachyOS's optimized scheduling."),

        // Session
        "seatd" => Some("Minimal seat management daemon. Provides unprivileged access to input/display devices for Wayland compositors (Sway, etc.)."),

        // VPN / Networking extras
        "openvpn-client" | "openvpn-server" => Some("OpenVPN tunnel. Template unit — instantiate with config name (e.g., openvpn-client@myconfig)."),
        "dnsmasq" => Some("Lightweight DNS forwarder and DHCP server. Often used for local DNS caching, network boot (PXE), or VM networking."),
        "nextdns" => Some("NextDNS CLI client. Routes DNS queries through NextDNS for ad-blocking, tracking protection, and security filtering."),

        _ => None,
    }
}

#[derive(Debug, Clone)]
pub enum ChangeAction {
    Enable,
    Disable,
}

#[derive(Debug, Clone)]
pub struct PendingChange {
    pub service: String,
    pub scope: ServiceScope,
    pub action: ChangeAction,
}

#[derive(Debug)]
pub struct ChangeResult {
    pub service: String,
    pub success: bool,
    pub message: String,
}

/// Apply changes using async commands with a timeout per command.
/// Separates enable/disable from start/stop so the enable always succeeds
/// even if the service is slow to start.
pub async fn apply_changes(changes: Vec<PendingChange>) -> Vec<ChangeResult> {
    let mut results = Vec::new();

    for change in &changes {
        let (enable_action, start_action) = match change.action {
            ChangeAction::Enable => ("enable", "start"),
            ChangeAction::Disable => ("disable", "stop"),
        };

        // Step 1: enable/disable (should be instant)
        let enable_result = run_systemctl(&change.scope, enable_action, &change.service).await;
        match enable_result {
            Ok(output) if output.status.success() => {
                // Step 2: start/stop (might be slow, use timeout)
                let start_result =
                    run_systemctl(&change.scope, start_action, &change.service).await;
                match start_result {
                    Ok(output) if output.status.success() => {
                        results.push(ChangeResult {
                            service: change.service.clone(),
                            success: true,
                            message: format!("{}d and {}ed", enable_action, start_action),
                        });
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        results.push(ChangeResult {
                            service: change.service.clone(),
                            success: false,
                            message: format!(
                                "{}d but {} failed: {}",
                                enable_action, start_action, stderr
                            ),
                        });
                    }
                    Err(e) => {
                        results.push(ChangeResult {
                            service: change.service.clone(),
                            success: false,
                            message: format!(
                                "{}d but {} timed out: {}",
                                enable_action, start_action, e
                            ),
                        });
                    }
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                results.push(ChangeResult {
                    service: change.service.clone(),
                    success: false,
                    message: format!("{} failed: {}", enable_action, stderr),
                });
            }
            Err(e) => {
                results.push(ChangeResult {
                    service: change.service.clone(),
                    success: false,
                    message: format!("{} timed out: {}", enable_action, e),
                });
            }
        }
    }

    results
}

async fn run_systemctl(
    scope: &ServiceScope,
    action: &str,
    service: &str,
) -> Result<std::process::Output, String> {
    let mut cmd = match scope {
        ServiceScope::User => {
            let mut c = AsyncCommand::new("systemctl");
            c.args(["--user", action, service]);
            c
        }
        ServiceScope::System => {
            let mut c = AsyncCommand::new("pkexec");
            c.args(["systemctl", action, service]);
            c
        }
    };

    match timeout(CMD_TIMEOUT, cmd.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(format!("command failed: {}", e)),
        Err(_) => {
            // Timeout — try to kill the child if possible
            Err("timed out after 10s".to_string())
        }
    }
}
