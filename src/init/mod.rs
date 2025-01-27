use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4}, path::PathBuf, time::Duration
};

use arti_client::{TorClient, TorClientBuilder};
use tokio::net::{TcpListener, TcpSocket};
use tor_rtcompat::{tokio::PreferredRuntime, PreferredRuntime};
use vec1::Vec1;

use crate::{
    controller::controller_api::ControllerConfig, main_process::{connection::ConnectionConfig, process::ProcessConfig}, renames::TorSocket, tools::{permission_list::PermissionList, traffic_rate::TrafficRate}, types::client_addr::ClientAddr
};

pub fn get_init() -> Init {
	// first, get the arguments from the user call,

	// then, we must search for the base configuration file, and if none is found, create a new one with a few defaults.
	
	todo!()
}


pub struct InitBuilder {
    conn_configs			: ConnConfigBuilder,
    permission_list			: Option<PermissionList<ClientAddr>>,
    max_simultaneous_clients: Option<u64>,

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
    conn_mngr_cfgs			: ProcessConfig,
    conn_cfgs				: ConnectionConfig,
    ctrl_cfgs               : ControllerConfig,      

}



impl Init {
    pub fn build_configs(&self) -> (ProcessConfig, ConnectionConfig) {
        (self.conn_mngr_cfgs.clone(), self.conn_cfgs.clone())
    }
    pub fn build_client_socket(&self) -> Result<Vec<TcpListener>, anyhow::Error> {
		let socket_ports = vec![413, 80];

        let sockets = 
            socket_ports
            .into_iter()
            .map(|port| -> Result<TcpListener, anyhow::Error>{
                let socket = TcpSocket::new_v4()?;
                socket.bind(
                    SocketAddr::V4( SocketAddrV4::new(
                        Ipv4Addr::UNSPECIFIED,
                        port 
                    ) )
                )?;
                Ok(socket.listen(self.conn_mngr_cfgs)?)
            })
        ;

        let listener = socket.listen(self.max_simultaneous_clients)?;
		Ok(listener)
    }
    pub fn build_host_socket(&self) -> Result<TorSocket, anyhow::Error> {

    }
}

// Simple onion to tcp service router
#[derive(clap::Parser)]
pub struct InitArgs {
    // Port that will receive the requests
    // The tor host to which the requests will be forwarded to
    #[arg(long="config-file", short='c', default_value_t=)]
    default_configs_file: PathBuf,

}
