use anyhow::Result;

use crate::constants::APP_DISPLAY_NAME;
use crate::style;
use crate::templates::all_templates;

pub fn run() -> Result<()> {
    style::heading(&format!("{APP_DISPLAY_NAME} templates"));
    for template in all_templates() {
        style::heading(template.name);
        style::info(template.description);
        style::field("Subnet", template.server.subnet);
        style::field(
            "Redirect GW",
            if template.server.redirect_gateway {
                "Enabled"
            } else {
                "Disabled"
            },
        );
        style::field(
            "Client-to-Client",
            if template.server.client_to_client.unwrap_or(false) {
                "Enabled"
            } else {
                "Disabled"
            },
        );
        if let Some(dns) = &template.server.dns {
            style::field("DNS Servers", dns.join(", "));
        }
    }
    println!();
    Ok(())
}
