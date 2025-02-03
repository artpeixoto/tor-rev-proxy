use arti_client::{BootstrapBehavior, TorClient, TorClientConfig};

use crate::renames::TorSocket;


pub async fn build_host_socket() -> Result<TorSocket, anyhow::Error>{
	let tor_client = 
		TorClient::builder()
			.config(TorClientConfig::builder().build()?)
			.bootstrap_behavior(BootstrapBehavior::Manual)
			.create_bootstrapped()
			.await?;

	Ok(tor_client)
}

