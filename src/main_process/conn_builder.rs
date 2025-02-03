use serde::{Deserialize, Serialize};

use crate::{host::HostAddrGetter, renames::*, types::client_addr::ClientAddr};

use super::{
    connection::{
        ConnectionConfig, ConnectionController, ConnectionEvent, ConnectionEventSender,
        ConnectionTask,
    },
    sockets::{client_sockets::ClientStream, host_socket::{self, build_host_socket}},
};
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BuilderEvent {
    CreatingHostSocket,
	HostSocketCreated,
    HostSocketConnecting,
    FullConnectionBuilt,
}
pub struct ConnectionBuilder {
	event_sender			: BroadcastSender<BuilderEvent>,
    conn_config_reader		: WatchReceiver<ConnectionConfig>,
    conn_global_event_sender: BroadcastSender<(ClientAddr, ConnectionEvent)>,
    host_addr_getter		: HostAddrGetter,
    host_socket				: TorSocket,
}

impl ConnectionBuilder {
    pub async fn new(
		event_sender: BroadcastSender<BuilderEvent>,
        host_addr_getter: HostAddrGetter,
        conn_config_reader: WatchReceiver<ConnectionConfig>,
        conn_evt_sender: BroadcastSender<(ClientAddr, ConnectionEvent)>,
    ) -> Result<Self, anyhow::Error> {
		let _ = event_sender.send(BuilderEvent::CreatingHostSocket);
        let host_socket = build_host_socket().await?;
		let _ = event_sender.send(BuilderEvent::HostSocketCreated);
        Ok(Self {
			event_sender,
            conn_config_reader,
            conn_global_event_sender: conn_evt_sender,
            host_addr_getter,
            host_socket,
        })
    }
    pub async fn build_connection(
        &mut self,
        client_stream: ClientStream,
        client_addr: ClientAddr,
    ) -> Result<(ConnectionController, ConnectionTask), anyhow::Error> {
		let _ = self.event_sender.send(BuilderEvent::HostSocketConnecting);
        let host_stream = self
            .host_socket
            .connect((self.host_addr_getter.get_host_addr().as_ref()))
            .await?;


        let conn = ConnectionTask::new(
            ConnectionEventSender::new(client_addr.clone(), self.conn_global_event_sender.clone()),
            self.conn_config_reader.clone(),
            client_stream,
            host_stream,
            self.host_addr_getter.clone(),
        );

        let conn_ctrl = ConnectionController::new(client_addr, Vec::new());
		let _ = self.event_sender.send(BuilderEvent::FullConnectionBuilt);
        Ok((conn_ctrl, conn))
    }
}
