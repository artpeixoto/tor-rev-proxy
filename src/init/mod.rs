use tokio::net::TcpSocket;

use crate::{main_process::{connection::ConnectionCfgs, connection_manager::ConnectionManagerCfgs}, renames::TorSocket, tools::permission_list::PermissionList};
pub struct InitBuilder{

}
impl InitBuilder{
	pub fn overlay(self, on: &mut Self) {
		todo!()	
	}

	pub fn build(self) -> Result<Init, anyhow::Error>{
		todo!()
	}
}
pub struct Init{
		
}

impl Init{
	
	pub fn build_configs(&self) -> (ConnectionManagerCfgs, ConnectionCfgs) {
		let mngr_cfgs = ConnectionManagerCfgs{
			permission_list: Default::default()
		};
		let conn_cfgs = ConnectionCfgs{
			traffic_max_rate	: todo!(),
			poll_rate_duration	: todo!(),
			timeout				: todo!(),
		};
		todo!()	
	}
	pub fn build_client_socket(&self) -> TcpSocket{
		todo!()
	}
	pub fn build_host_socket(&self) -> TorSocket{
		todo!()
	}
}

pub fn get_init() -> Init{
	todo!()
}



// Simple onion to tcp service router
#[derive(clap::Parser)]
pub struct InitArgs{
    // Port that will receive the requests
    #[arg(short='p', long="serve-port", default_value_t=80) ]    
    pub serve_port  : u16,

    // The tor host to which the requests will be forwarded to 
    #[arg(short='H', long="host" , default_value_t=("haystak5njsmn2hqkewecpaxetahtwhsbsa64jom2k22z5afxhnpxfid.onion".to_string()))]
    pub host        : String,

        
    #[arg(long="host-port", short='P', default_value_t=80)]
    pub host_port   : u16
}

