pub const CATEGORY_ORDER: &[&str] = &[
    "Audio",
    "Bluetooth",
    "Containers",
    "Display",
    "Network",
    "Printing",
    "Security",
    "Systemd Core",
    "Other",
];

pub fn categorize(service_name: &str) -> &'static str {
    let name = service_name.trim_end_matches(".service");

    if matches_any(name, &[
        "NetworkManager", "wpa_supplicant", "systemd-networkd",
        "systemd-resolved", "iwd", "dhcpcd", "connman",
    ]) {
        return "Network";
    }

    if matches_any(name, &["pipewire", "pulseaudio", "wireplumber"]) {
        return "Audio";
    }

    if matches_any(name, &["bluetooth", "blueman"]) {
        return "Bluetooth";
    }

    if matches_any(name, &["gdm", "sddm", "lightdm", "greetd", "ly"]) {
        return "Display";
    }

    if matches_any(name, &["docker", "podman", "containerd"]) {
        return "Containers";
    }

    if matches_any(name, &["firewalld", "ufw", "apparmor", "sshd", "fail2ban"]) {
        return "Security";
    }

    if matches_any(name, &["cups", "avahi"]) {
        return "Printing";
    }

    if name.starts_with("systemd-") {
        return "Systemd Core";
    }

    "Other"
}

fn matches_any(name: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| name.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_network() {
        assert_eq!(categorize("NetworkManager.service"), "Network");
        assert_eq!(categorize("wpa_supplicant.service"), "Network");
    }

    #[test]
    fn test_categorize_audio() {
        assert_eq!(categorize("pipewire.service"), "Audio");
        assert_eq!(categorize("wireplumber.service"), "Audio");
    }

    #[test]
    fn test_categorize_systemd_core() {
        assert_eq!(categorize("systemd-journald.service"), "Systemd Core");
        assert_eq!(categorize("systemd-logind.service"), "Systemd Core");
    }

    #[test]
    fn test_categorize_systemd_network_overrides_core() {
        assert_eq!(categorize("systemd-networkd.service"), "Network");
        assert_eq!(categorize("systemd-resolved.service"), "Network");
    }

    #[test]
    fn test_categorize_unknown() {
        assert_eq!(categorize("my-custom-thing.service"), "Other");
    }
}
