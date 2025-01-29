use arti_client::{BootstrapBehavior, TorClient, TorClientConfig};

use crate::renames::TorSocket;


pub(super) fn build_host_socket() -> Result<TorSocket, anyhow::Error>{
	let tor_client = 
		TorClient::builder()
			.config(TorClientConfig::builder().build()?)
			.bootstrap_behavior(BootstrapBehavior::OnDemand)
			.create_unbootstrapped()?;

	Ok(tor_client)
}

