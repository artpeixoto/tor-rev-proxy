use crate::{init::Init, main_process::{connection::{ConnectionConfig, ConnectionEvent}, process::{ProcessEvent, Process, ProcessConfig}}, renames::*, types::client_addr::ClientAddr};
pub struct ProcessEndpoint{
	pub cfg_readers: ConfigReaders,
	pub evt_senders: EventSenders
}

pub struct ControllerEndpoint{
	pub cfg: ConfigWriters,
	pub evt: EventListeners,
}

pub fn make_comm_channels(initial_configurations: (ProcessConfig, ConnectionConfig)) -> (ProcessEndpoint, ControllerEndpoint ){

	let (conn_mngr_cfg_sender, conn_mngr_cfg_receiver)  = watch_channel(initial_configurations.0);
    let (conn_cfg_sender, conn_cfg_receiver)            = watch_channel(initial_configurations.1);

    let (conn_evt_sender, conn_evt_receiver)            = broadcast_channel(16 * 1024);
    let (conn_mngr_evt_sender, conn_mngr_evt_receiver)  = broadcast_channel(1024);
	(
		ProcessEndpoint{
			cfg_readers: ConfigReaders 	{ 
				conn: conn_cfg_receiver, 
				process: conn_mngr_cfg_receiver 
			},
			evt_senders: EventSenders{ 
				conn: conn_evt_sender	 , 
				process: conn_mngr_evt_sender  
			}
		},
		ControllerEndpoint{
			cfg: ConfigWriters{
				conn		: conn_cfg_sender,
				process	: conn_mngr_cfg_sender
			},
			evt: EventListeners{
				conn		: conn_evt_receiver,
				process	: conn_mngr_evt_receiver,
			}
		}
	)
}


pub struct ConfigReaders{
	pub conn	: WatchReceiver<ConnectionConfig>,
	pub process	: WatchReceiver<ProcessConfig>
}
pub struct ConfigWriters{
	pub conn	: WatchSender<ConnectionConfig>,
	pub process	: WatchSender<ProcessConfig>
}

pub struct EventSenders{
	pub conn		: BroadcastSender<(ClientAddr, ConnectionEvent)>,
	pub process	: BroadcastSender<ProcessEvent>
}


pub struct EventListeners{
	pub conn	: BroadcastReceiver<(ClientAddr, ConnectionEvent)>,
	pub process	: BroadcastReceiver<ProcessEvent>
}


impl Clone for EventListeners{
	fn clone(&self) -> Self {
		Self { 
			conn: self.conn.resubscribe(), 
			process: self.process.resubscribe() 
		}
	}
}