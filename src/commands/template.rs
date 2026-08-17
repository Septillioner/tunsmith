use anyhow::Result;

use crate::constants::APP_DISPLAY_NAME;
use crate::templates::all_templates;

pub fn run() -> Result<()> {
    println!("\nAvailable {APP_DISPLAY_NAME} templates:");
    for template in all_templates() {
        println!("\n{}:", template.name);
        println!("  {}", template.description);
        println!("  Subnet:          {}", template.server.subnet);
        println!(
            "  Redirect GW:     {}",
            if template.server.redirect_gateway {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        println!(
            "  Client-to-Client: {}",
            if template.server.client_to_client.unwrap_or(false) {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        if let Some(dns) = &template.server.dns {
            println!("  DNS Servers:     {}", dns.join(", "));
        }
    }
    println!();
    Ok(())
}
