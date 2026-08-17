use crate::project::ServerConfig;

pub const TEMPLATE_GATEWAY_VPN: &str = "gateway-vpn";
pub const TEMPLATE_CLOUD_VPN: &str = "cloud-vpn";
pub const TEMPLATE_GATEWAY_CLOUD_VPN: &str = "gateway-cloud-vpn";

pub struct Template {
    pub name: &'static str,
    pub description: &'static str,
    pub server: ServerConfig,
}

pub fn all_templates() -> Vec<Template> {
    vec![
        Template {
            name: TEMPLATE_GATEWAY_VPN,
            description: "Full tunnel VPN. Redirects all client traffic through the VPN server.",
            server: ServerConfig {
                redirect_gateway: true,
                client_to_client: Some(false),
                block_outside_dns: Some(true),
                subnet: "10.8.0.0/24".to_string(),
                dns: Some(crate::constants::default_dns()),
                ..ServerConfig::default()
            },
        },
        Template {
            name: TEMPLATE_CLOUD_VPN,
            description:
                "Split tunnel VPN. Best for accessing private networks with client communication.",
            server: ServerConfig {
                redirect_gateway: false,
                client_to_client: Some(true),
                block_outside_dns: Some(false),
                subnet: "10.10.0.0/24".to_string(),
                ..ServerConfig::default()
            },
        },
        Template {
            name: TEMPLATE_GATEWAY_CLOUD_VPN,
            description: "Full tunnel VPN with client-to-client communication enabled.",
            server: ServerConfig {
                redirect_gateway: true,
                client_to_client: Some(true),
                block_outside_dns: Some(true),
                subnet: "10.12.0.0/24".to_string(),
                dns: Some(crate::constants::default_dns()),
                ..ServerConfig::default()
            },
        },
    ]
}

pub fn find_template(name: &str) -> Option<Template> {
    all_templates().into_iter().find(|t| t.name == name)
}

pub fn template_names() -> String {
    all_templates()
        .iter()
        .map(|t| t.name)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::DEFAULT_SUBNET;

    #[test]
    fn templates_exist_and_gateway_differs_from_cloud() {
        let gateway = find_template(TEMPLATE_GATEWAY_VPN).expect("gateway-vpn template");
        let cloud = find_template(TEMPLATE_CLOUD_VPN).expect("cloud-vpn template");
        let gateway_cloud =
            find_template(TEMPLATE_GATEWAY_CLOUD_VPN).expect("gateway-cloud-vpn template");

        assert!(gateway.server.redirect_gateway);
        assert!(!cloud.server.redirect_gateway);
        assert!(gateway_cloud.server.redirect_gateway);
        assert_ne!(gateway.server.subnet, cloud.server.subnet);
        assert_ne!(cloud.server.subnet, gateway_cloud.server.subnet);
        assert_eq!(gateway.server.subnet, DEFAULT_SUBNET);
        assert_eq!(
            gateway.server.dns.as_ref(),
            Some(&crate::constants::default_dns())
        );
        assert!(cloud.server.dns.is_none());
        assert!(cloud.server.client_to_client.unwrap_or(false));
        assert!(!gateway.server.client_to_client.unwrap_or(false));
    }

    #[test]
    fn template_names_lists_all_known_templates() {
        let names = template_names();
        assert!(names.contains(TEMPLATE_GATEWAY_VPN));
        assert!(names.contains(TEMPLATE_CLOUD_VPN));
        assert!(names.contains(TEMPLATE_GATEWAY_CLOUD_VPN));
        assert!(find_template("no-such-template").is_none());
    }
}
