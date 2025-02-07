use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::select;
use crate::{main_process::{conn_builder::BuilderEvent, connection::ConnectionEvent, listener::ListenerEvent}, renames::BroadcastReceiver, types::client_addr::ClientAddr};
pub struct EventLogger{
	pub connection_events_receiver	: BroadcastReceiver<(ClientAddr, ConnectionEvent)>,
	pub builder_events_receiver		: BroadcastReceiver<BuilderEvent>,
	pub listener_events_receiver	: BroadcastReceiver<ListenerEvent>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Event{
	ConnectionEvent{
		client_addr:ClientAddr,
		event: ConnectionEvent,
	},
	BuilderEvent (BuilderEvent),
	ListenerEvent(ListenerEvent)
}

impl From<(ClientAddr, ConnectionEvent)> for Event{
	fn from(value: (ClientAddr, ConnectionEvent)) -> Self {
		Event::ConnectionEvent { client_addr: value.0, event: value.1 }
	}
}

impl From<BuilderEvent> for Event{
	fn from(value: BuilderEvent) -> Self {
		Self::BuilderEvent(value)
	}
}
impl From<ListenerEvent> for Event{
	fn from(value: ListenerEvent) -> Self {
		Self::ListenerEvent(value)
	}
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
					info!("Connection {ip_addr:?}\t> {evt:?}");
				},
				recv = self.builder_events_receiver.recv() => {
					let mngr_evt = recv.unwrap();
					info!("{mngr_evt:?}");
				},
				recv = self.listener_events_receiver.recv() => {
					let listener_evt = recv.unwrap();
					info!("{listener_evt:?}");
				}
			}
		} Ok(())}
		.await;

		error!("Event logger has failed: {break_reason:?}");

	}
}