use std::{
    collections::HashSet, net::{Ipv4Addr, SocketAddr, SocketAddrV4}, path::PathBuf, str::FromStr, time::Duration
};

use arti_client::{TorClient, TorClientBuilder};
use tokio::net::{TcpListener, TcpSocket};
use vec1::Vec1;

use crate::{
    main_process::{manager::ClientsPermissionList, connection::ConnectionConfig, listener::{ClientSocketsConfig, Listener}, sockets::client_sockets::SocketConfig}, renames::TorSocket, tools::{permission_list::PermissionList, traffic_limiter::TrafficRate }, types::client_addr::ClientAddr
};

pub fn get_init() -> Init {
	// first, get the arguments from the user call,

	// then, we must search for the base configuration file, and if none is found, create a new one with a few defaults.
	
    Init { 
        permission_list: PermissionList::Block(HashSet::new()),
        conn_cfgs: ConnectionConfig { 
            traffic_max_rate: TrafficRate::new(3000), 
            poll_rate_duration: Duration::from_secs_f32(0.1), 
            timeout: Duration::from_secs(5), 
        }, 
        client_sockets: ClientSocketsConfig{
            client_tcp_socket_config: Some(SocketConfig{
                max_clients: 1024,
                port: 80,
            }),
            client_tls_socket_config: None,
        }
    }
}


pub struct InitBuilder {
    conn_configs			: ConnConfigBuilder,
    permission_list			: Option<PermissionList<ClientAddr>>,
    sockets_configs         : Option<u64>,
}


pub struct ConnConfigBuilder {
    traffic_max_rate    : Option<TrafficRate>,
    timeout             : Option<Duration>,
    poll_rate_duration  : Option<Duration>,
}


impl InitBuilder {
    pub fn new_empty() -> Self {
        todo!()
    }
    pub fn overlay(self, on: &mut Self) {
        todo!()
    }

    pub fn build(self) -> Result<Init, anyhow::Error> {
        todo!()
    }

    pub fn build_with_defaults(self, defaults: Init) -> Init {
        todo!()
    }
}

pub struct Init {
    pub permission_list			: ClientsPermissionList,
    pub conn_cfgs				: ConnectionConfig,
    pub client_sockets          : ClientSocketsConfig,      
}



impl Init {
    
}

// Simple onion to tcp service router
#[derive(clap::Parser)]
pub struct InitArgs {

    // Port that will receive the requests
    // The tor host to which the requests will be forwarded to
    #[arg(
        long="config-file", 
        short='c', 
        default_value_t={"./rev_proxy_configs.yaml".to_string()}
    )]
    default_configs_file: String,
}
