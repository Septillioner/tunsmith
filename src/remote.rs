use anyhow::{bail, Result};
use std::path::Path;

use crate::constants::{
    IFACE_NAME_MAX, IPV4_MAX_PREFIX, NAT_COMMENT_PREFIX, NAT_DOWN_SCRIPT_NAME, NAT_SCRIPT_NAME,
    NAT_SYSTEMD_TEMPLATE, NAT_SYSTEMD_UNIT_DIR, NAT_TUN_MATCH, REMOTE_OPENVPN_SERVER_DIR,
    REMOTE_SCRIPT_MODE, REMOTE_UNIT_MODE, SYSCTL_CONF,
};
use crate::project::now_rfc3339;
use crate::ssh::{journal_tail_command, RemoteSession};

pub struct RemoteInfo {
    pub os: String,
    pub kernel: String,
    pub vpn_version: String,
    pub hostname: String,
    pub is_forwarding_enabled: bool,
    pub cpu: String,
    pub ram: String,
    pub disk: String,
    pub uptime: String,
    pub public_ip: String,
    pub local_ip: String,
}

pub struct VpnManager<'a> {
    session: &'a RemoteSession,
}

impl<'a> VpnManager<'a> {
    pub fn new(session: &'a RemoteSession) -> Self {
        Self { session }
    }

    pub async fn analyze_environment(&self, default_hostname: &str) -> Result<RemoteInfo> {
        Ok(RemoteInfo {
            vpn_version: self.vpn_version().await,
            os: self
                .session
                .execute_or(
                    "cat /etc/os-release | grep PRETTY_NAME | cut -d= -f2 | tr -d '\"'",
                    "Unknown Linux",
                )
                .await,
            kernel: self.session.execute_or("uname -sr", "Unknown").await,
            hostname: {
                let name = self.session.execute_or("hostname", default_hostname).await;
                if name.is_empty() {
                    default_hostname.to_string()
                } else {
                    name
                }
            },
            is_forwarding_enabled: self.check_ip_forwarding().await,
            cpu: self
                .session
                .execute_or(
                    "grep \"model name\" /proc/cpuinfo | head -n 1 | cut -d: -f2 | xargs",
                    "Unknown",
                )
                .await,
            ram: self
                .session
                .execute_or(
                    "free -h | grep Mem | awk '{print $2 \" Total (\" $7 \" available)\"}'",
                    "Unknown",
                )
                .await,
            disk: self
                .session
                .execute_or(
                    "df -h / | tail -n 1 | awk '{print $2 \" Total (\" $4 \" free)\"}'",
                    "Unknown",
                )
                .await,
            uptime: self.session.execute_or("uptime -p", "Unknown").await,
            public_ip: self
                .session
                .execute_or("curl -s https://ifconfig.me", "Unknown")
                .await,
            local_ip: self
                .session
                .execute_or("hostname -I | awk '{print $1}'", "Unknown")
                .await,
        })
    }

    pub async fn vpn_version(&self) -> String {
        let direct = self
            .session
            .execute_or("openvpn --version | head -n 1", "")
            .await;
        if !direct.is_empty() {
            return direct;
        }
        let dpkg = self
            .session
            .execute_or("dpkg -s openvpn | grep Version | cut -d: -f2", "")
            .await;
        if !dpkg.is_empty() {
            return dpkg.trim().to_string();
        }
        let rpm = self
            .session
            .execute_or("rpm -q openvpn --queryformat \"%{VERSION}\"", "")
            .await;
        if rpm.is_empty() || rpm.contains("not installed") {
            "Not installed".to_string()
        } else {
            rpm
        }
    }

    pub async fn check_ip_forwarding(&self) -> bool {
        self.session
            .execute_or("cat /proc/sys/net/ipv4/ip_forward", "0")
            .await
            .trim()
            == "1"
    }

    pub async fn enable_ip_forwarding(&self) -> Result<()> {
        self.session
            .execute("sysctl -w net.ipv4.ip_forward=1")
            .await?;
        self.session
            .execute(&format!(
                "sed -i '/^net.ipv4.ip_forward/d' {SYSCTL_CONF} && echo \"net.ipv4.ip_forward=1\" >> {SYSCTL_CONF}"
            ))
            .await?;
        Ok(())
    }

    pub async fn ensure_dependencies(&self, mut logs: impl FnMut(&str)) -> Result<()> {
        logs("Checking dependencies...");
        let vpn_ver = self.vpn_version().await;
        if vpn_ver != "Not installed" {
            logs(&format!("OpenVPN is already installed ({vpn_ver})."));
            return Ok(());
        }

        logs("Installing OpenVPN...");
        let os_info = self
            .session
            .execute_or(
                "cat /etc/os-release | grep PRETTY_NAME | cut -d= -f2 | tr -d '\"'",
                "",
            )
            .await;
        if os_info.contains("Debian") || os_info.contains("Ubuntu") {
            self.session
                .execute("apt-get update && apt-get install -y openvpn")
                .await?;
        } else {
            bail!(
                "Unsupported OS ({os_info}) for automatic installation. Install OpenVPN manually."
            );
        }
        logs("OpenVPN installed successfully.");
        Ok(())
    }

    pub async fn setup_vpn(
        &self,
        instance_name: &str,
        local_dist_dir: &Path,
        mut logs: impl FnMut(&str),
    ) -> Result<()> {
        let remote_config_dir = format!("{REMOTE_OPENVPN_SERVER_DIR}/{instance_name}");
        let service_name = format!("openvpn-server@{instance_name}");

        logs(&format!(
            "Creating configuration directory: {remote_config_dir}"
        ));
        self.session.mkdir_p(&remote_config_dir).await?;

        let files = [
            ("server.conf", format!("{instance_name}.conf")),
            ("ca.crt", "ca.crt".to_string()),
            ("server.crt", "server.crt".to_string()),
            ("server.key", "server.key".to_string()),
            ("tls-crypt.key", "tls-crypt.key".to_string()),
        ];

        for (local_name, remote_name) in files {
            let local_path = local_dist_dir.join(local_name);
            if local_path.is_file() {
                let remote_path = format!("{remote_config_dir}/{remote_name}");
                logs(&format!("Uploading {local_name} -> {remote_path}"));
                self.session.upload_file(&local_path, &remote_path).await?;
            }
        }

        let main_config_path = format!("{REMOTE_OPENVPN_SERVER_DIR}/{instance_name}.conf");
        logs(&format!("Deploying main config to {main_config_path}"));
        self.session
            .execute(&format!(
                "cp {remote_config_dir}/{instance_name}.conf {main_config_path}"
            ))
            .await?;

        logs(&format!(
            "Starting and enabling OpenVPN service ({service_name})..."
        ));
        self.session
            .execute(&format!("systemctl enable {service_name}"))
            .await?;
        self.session
            .execute(&format!("systemctl restart {service_name}"))
            .await?;

        let status = self
            .session
            .execute_or(&format!("systemctl is-active {service_name}"), "inactive")
            .await;
        if status != "active" {
            let journal = self
                .session
                .execute_or(
                    &journal_tail_command(&service_name),
                    "Could not retrieve logs",
                )
                .await;
            bail!("OpenVPN service failed to start (Status: {status}).\n--- Remote Logs ---\n{journal}");
        }

        logs("OpenVPN service is active and running.");
        Ok(())
    }

    pub async fn default_ipv4_ifaces(&self) -> Vec<String> {
        let raw = self
            .session
            .execute_or("ip -4 route show default", "")
            .await;
        parse_default_route_ifaces(&raw)
    }

    pub async fn ufw_is_active(&self) -> bool {
        let status = self
            .session
            .execute_or("ufw status 2>/dev/null | head -n 1", "")
            .await;
        status.to_ascii_lowercase().contains("status: active")
    }

    pub async fn apply_gateway_nat(
        &self,
        instance_name: &str,
        subnet: &str,
        iface: &str,
        mut logs: impl FnMut(&str),
    ) -> Result<()> {
        crate::project::validate_instance_name(instance_name)?;
        validate_iface(iface)?;
        validate_ipv4_cidr(subnet)?;

        let remote_dir = format!("{REMOTE_OPENVPN_SERVER_DIR}/{instance_name}");
        let comment = format!("{NAT_COMMENT_PREFIX}{instance_name}");
        let nat_sh = nat_apply_script(subnet, iface, &comment);
        let nat_down = nat_down_script(subnet, iface, &comment);
        let unit = nat_systemd_unit();

        logs("Uploading NAT scripts...");
        self.session
            .upload_bytes(
                nat_sh.as_bytes(),
                &format!("{remote_dir}/{NAT_SCRIPT_NAME}"),
                REMOTE_SCRIPT_MODE,
            )
            .await?;
        self.session
            .upload_bytes(
                nat_down.as_bytes(),
                &format!("{remote_dir}/{NAT_DOWN_SCRIPT_NAME}"),
                REMOTE_SCRIPT_MODE,
            )
            .await?;
        self.session
            .upload_bytes(
                unit.as_bytes(),
                &format!("{NAT_SYSTEMD_UNIT_DIR}/{NAT_SYSTEMD_TEMPLATE}"),
                REMOTE_UNIT_MODE,
            )
            .await?;

        let unit_instance = format!("tunsmith-nat@{instance_name}");
        logs(&format!("Enabling {unit_instance}..."));
        self.session.execute("systemctl daemon-reload").await?;
        self.session
            .execute(&format!("systemctl enable {unit_instance}"))
            .await?;
        self.session
            .execute(&format!("systemctl restart {unit_instance}"))
            .await?;
        logs(&format!("NAT masquerade applied on {iface} for {subnet}."));
        Ok(())
    }

    pub async fn remove_gateway_nat(&self, instance_name: &str, mut logs: impl FnMut(&str)) {
        if crate::project::validate_instance_name(instance_name).is_err() {
            return;
        }
        let unit_instance = format!("tunsmith-nat@{instance_name}");
        logs(&format!("Stopping {unit_instance}..."));
        let _ = self
            .session
            .execute(&format!("systemctl stop {unit_instance}"))
            .await;
        let _ = self
            .session
            .execute(&format!("systemctl disable {unit_instance}"))
            .await;
        let down = format!("{REMOTE_OPENVPN_SERVER_DIR}/{instance_name}/{NAT_DOWN_SCRIPT_NAME}");
        let _ = self
            .session
            .execute(&format!("test -x '{down}' && '{down}'"))
            .await;
    }

    pub async fn update_config(
        &self,
        instance_name: &str,
        local_config: &Path,
        mut logs: impl FnMut(&str),
    ) -> Result<()> {
        let remote_config_path = format!("{REMOTE_OPENVPN_SERVER_DIR}/{instance_name}.conf");
        let service_name = format!("openvpn-server@{instance_name}");
        logs(&format!("Uploading {remote_config_path}"));
        self.session
            .upload_file(local_config, &remote_config_path)
            .await?;
        logs(&format!("Restarting {service_name}"));
        self.session
            .execute(&format!("systemctl restart {service_name}"))
            .await?;
        Ok(())
    }

    pub async fn cleanup_vpn(&self, instance_name: &str, mut logs: impl FnMut(&str)) -> Result<()> {
        self.remove_gateway_nat(instance_name, &mut logs).await;

        let service_name = format!("openvpn-server@{instance_name}");
        let remote_config_dir = format!("{REMOTE_OPENVPN_SERVER_DIR}/{instance_name}");
        let main_config_path = format!("{REMOTE_OPENVPN_SERVER_DIR}/{instance_name}.conf");

        logs(&format!("Stopping and disabling service: {service_name}"));
        let _ = self
            .session
            .execute(&format!("systemctl stop {service_name}"))
            .await;
        let _ = self
            .session
            .execute(&format!("systemctl disable {service_name}"))
            .await;

        logs("Removing configuration files...");
        let _ = self
            .session
            .execute(&format!("rm -f {main_config_path}"))
            .await;
        let _ = self
            .session
            .execute(&format!("rm -rf {remote_config_dir}"))
            .await;

        logs(&format!(
            "Instance {instance_name} cleaned up successfully."
        ));
        Ok(())
    }
}

pub fn deployed_at() -> String {
    now_rfc3339()
}

pub fn parse_default_route_ifaces(raw: &str) -> Vec<String> {
    let mut ifaces = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let mut index = 0;
        while index + 1 < parts.len() {
            if parts[index] == "dev" {
                let name = parts[index + 1];
                if !ifaces.iter().any(|existing| existing == name) {
                    ifaces.push(name.to_string());
                }
                index += 2;
                continue;
            }
            index += 1;
        }
    }
    ifaces
}

pub fn validate_iface(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > IFACE_NAME_MAX {
        bail!("invalid network interface name");
    }
    let valid = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');
    if !valid {
        bail!("invalid network interface name");
    }
    Ok(())
}

pub fn validate_ipv4_cidr(subnet: &str) -> Result<()> {
    let (addr, prefix) = subnet
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("subnet must be IPv4 CIDR, e.g. 10.8.0.0/24"))?;
    let bits: u32 = prefix
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid subnet prefix"))?;
    if bits > IPV4_MAX_PREFIX {
        bail!("invalid subnet prefix");
    }
    let octets: Vec<&str> = addr.split('.').collect();
    if octets.len() != 4 {
        bail!("invalid IPv4 subnet");
    }
    for octet in octets {
        if octet.len() > 1 && octet.starts_with('0') {
            bail!("invalid IPv4 subnet");
        }
        let value: u32 = octet
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid IPv4 subnet"))?;
        if value > 255 {
            bail!("invalid IPv4 subnet");
        }
    }
    Ok(())
}

fn nat_apply_script(subnet: &str, iface: &str, comment: &str) -> String {
    let check_nat = iptables_nat_spec(subnet, iface, comment);
    let check_fwd_out = iptables_forward_out(iface);
    let check_fwd_in = iptables_forward_in(iface);
    format!(
        "#!/bin/sh\n\
set -e\n\
iptables -t nat -C {check_nat} 2>/dev/null || iptables -t nat -A {check_nat}\n\
iptables -C {check_fwd_out} 2>/dev/null || iptables -A {check_fwd_out}\n\
iptables -C {check_fwd_in} 2>/dev/null || iptables -A {check_fwd_in}\n"
    )
}

fn nat_down_script(subnet: &str, iface: &str, comment: &str) -> String {
    let check_nat = iptables_nat_spec(subnet, iface, comment);
    let check_fwd_out = iptables_forward_out(iface);
    let check_fwd_in = iptables_forward_in(iface);
    format!(
        "#!/bin/sh\n\
iptables -t nat -C {check_nat} 2>/dev/null && iptables -t nat -D {check_nat} || true\n\
iptables -C {check_fwd_out} 2>/dev/null && iptables -D {check_fwd_out} || true\n\
iptables -C {check_fwd_in} 2>/dev/null && iptables -D {check_fwd_in} || true\n"
    )
}

fn iptables_nat_spec(subnet: &str, iface: &str, comment: &str) -> String {
    format!("POSTROUTING -s '{subnet}' -o '{iface}' -m comment --comment '{comment}' -j MASQUERADE")
}

fn iptables_forward_out(iface: &str) -> String {
    format!("FORWARD -i '{NAT_TUN_MATCH}' -o '{iface}' -j ACCEPT")
}

fn iptables_forward_in(iface: &str) -> String {
    format!(
        "FORWARD -i '{iface}' -o '{NAT_TUN_MATCH}' -m state --state RELATED,ESTABLISHED -j ACCEPT"
    )
}

fn nat_systemd_unit() -> String {
    format!(
        "[Unit]\n\
Description=Tunsmith NAT masquerade for OpenVPN instance %i\n\
After=network-online.target\n\
Wants=network-online.target\n\
\n\
[Service]\n\
Type=oneshot\n\
RemainAfterExit=yes\n\
ExecStart={REMOTE_OPENVPN_SERVER_DIR}/%i/{NAT_SCRIPT_NAME}\n\
ExecStop={REMOTE_OPENVPN_SERVER_DIR}/%i/{NAT_DOWN_SCRIPT_NAME}\n\
\n\
[Install]\n\
WantedBy=multi-user.target\n\
WantedBy=openvpn-server@%i.service\n"
    )
}
