use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::{ sync::Arc};
use super::manager::ClientsPermissionList;
use super::sockets::client_sockets::{self, ClientSockets, ClientStream, SocketConfig, TlsAcceptorConfig};
use crate::{init, renames::*};
use crate::{
    types::client_addr::{self, ClientAddr},
};
use anyhow::{anyhow, Error};
use serde::{Deserialize, Serialize};
use tokio::select;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq )]
pub struct ClientSocketsConfig {
    pub client_tcp_socket_config : Option<SocketConfig>,
    pub client_tls_socket_config : Option<( SocketConfig, TlsAcceptorConfig)>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ListenerEvent{
    BuildingSockets,
    DroppingSockets,
    SocketsConfigChanged,
    SocketsConfigIs(),
    SocketsBuilt,
    ReceivedNewConnection,
    ConnectionAccepted,
    ConnectionDenied,
}


pub struct Listener {
    config_reader           : WatchReceiver<ClientSocketsConfig>,
    client_permission_list  : WatchReceiver<ClientsPermissionList>,
    event_sender            : BroadcastSender<ListenerEvent>,
    client_sockets          : ClientSockets,
    new_conns_sender        : MpscSender<(ClientStream, ClientAddr)>,
}


impl Listener {
    
    pub fn new(
        mut config_reader   : WatchReceiver<ClientSocketsConfig>,
        event_sender        : BroadcastSender<ListenerEvent>, 
        new_conns_sender    : MpscSender<(ClientStream, ClientAddr)>,
        client_permission_list  : WatchReceiver<ClientsPermissionList>,
    ) -> Result<Self, anyhow::Error> {
        let configs = config_reader.borrow_and_update();
        event_sender.send()
        let client_sockets = ClientSockets::build_client_sockets(&configs.client_tcp_socket_config, &configs.client_tls_socket_config).unwrap();
        drop(configs);

        Ok(Self {
            config_reader,
            event_sender ,
            client_sockets,
            new_conns_sender,
            client_permission_list ,
        })
    }
    pub async fn run(&mut self) -> Result<(), anyhow::Error>{
        loop {
            select!{
                accept_res = self.client_sockets.accept() => {
                    let (client_stream, client_socket_addr) = accept_res.unwrap();
                    self.handle_new_connection(client_stream, client_socket_addr).await.unwrap();
                },
            }
        }
    }
    
    pub async fn handle_new_connection(&mut self, client_stream: ClientStream, client_addr: SocketAddr) -> Result<(), anyhow::Error> {
        let _ = self.event_sender.send(ListenerEvent::ReceivedNewConnection);
        let client_addr = ClientAddr(client_addr.ip());
        if self.client_permission_list.borrow_and_update().allows(&client_addr){
            
            self.new_conns_sender.send((client_stream, client_addr)).await.unwrap(); 
        } else {

        }
        Ok(())
    }
}
