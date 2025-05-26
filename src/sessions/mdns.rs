use log::info;
#[cfg(feature = "mdns")]
use mdns_sd::{ServiceDaemon, ServiceInfo};

#[cfg(feature = "mdns")]
pub fn advertise_mdns(instance_name: &str, port: u16) -> Result<(), mdns_sd::Error> {
    let mdns = ServiceDaemon::new()?;
    let service_type = "_apple-midi._udp.local.";
    let ip = local_ip_address::local_ip().expect("Failed to get local IP address").to_string();

    let raw_hostname = hostname::get().expect("Failed to get hostname").to_string_lossy().to_string();
    let hostname = format!("{}.local.", raw_hostname);
    let service = ServiceInfo::new(service_type, instance_name, &hostname, ip, port, None)?;
    mdns.register(service)?;

    Ok(())
}

#[cfg(not(feature = "mdns"))]
pub fn advertise_mdns(_: &str, _: u16) -> Result<(), std::io::Error> {
    info!("mDNS advertising is disabled. To enable it, compile with the 'mdns' feature.");
    Ok(())
}
