use log::info;
use tokio::select;

use crate::{main_process::{conn_builder::BuilderEvent, connection::ConnectionEvent, listener::ListenerEvent}, renames::BroadcastReceiver, types::client_addr::ClientAddr};


pub struct EventLogger{
	pub connection_events_receiver	: BroadcastReceiver<(ClientAddr, ConnectionEvent)>,
	pub builder_events_receiver		: BroadcastReceiver<BuilderEvent>,
	pub listener_events_receiver	: BroadcastReceiver<ListenerEvent>,
}
#[derive(Clone, PartialEq, Eq, )]
pub enum Event{
	ConnectionEvent{
		client_addr:ClientAddr,
		event: ConnectionEvent,
	},
	BuilderEvent(BuilderEvent),
	ListenerEvent(ListenerEvent)
}


impl EventLogger{
	pub fn new(
		connection_events_receiver	: &BroadcastReceiver<(ClientAddr, ConnectionEvent)>,
		builder_events_receiver		: &BroadcastReceiver<BuilderEvent>,
		listener_events_receiver	: &BroadcastReceiver<ListenerEvent>,
	) -> Self {
		Self { 
			connection_events_receiver: connection_events_receiver.resubscribe(),
			builder_events_receiver: builder_events_receiver.resubscribe(),
			listener_events_receiver: listener_events_receiver.resubscribe(), 
		}
	}
	pub async fn run(&mut self) {
		let break_reason : Result<(), anyhow::Error> = async { loop { 
			select!{
				recv = self.connection_events_receiver.recv() => {
					let (ip_addr, evt) = recv.unwrap();
					info!("[Connection {ip_addr:?}]: {evt:?}");
				},
				recv = self.builder_events_receiver.recv() => {
					let mngr_evt = recv.unwrap();
					info!("[Manager]: {mngr_evt:?}");
				},
				recv = self.listener_events_receiver.recv() => {
					let listener_evt = recv.unwrap();
					info!("[Listener]: {listener_evt:?}");
				}
			}
		} Ok(())}
		.await;
	}
}