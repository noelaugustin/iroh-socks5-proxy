// Connection logger
use iroh::Watcher;
use iroh::endpoint::{ConnectionType, Endpoint};

/// Log connection details (Direct/Relay/Mixed)
pub fn log_connection_details(endpoint: &Endpoint, remote_node_id: iroh::PublicKey, prefix: &str) {
    if let Some(mut watcher) = endpoint.conn_type(remote_node_id) {
        let type_ = watcher.get();
        match type_ {
            ConnectionType::Direct(addr) => println!("{} (Mode: Direct, Addr: {})", prefix, addr),
            ConnectionType::Relay(url) => println!("{} (Mode: Relay, Url: {})", prefix, url),
            ConnectionType::Mixed(addr, url) => {
                println!("{} (Mode: Mixed, Addr: {}, Url: {})", prefix, addr, url)
            }
            ConnectionType::None => println!("{} (Mode: None)", prefix),
        }
    } else {
        println!("{} (Mode: Unknown)", prefix);
    }
}
