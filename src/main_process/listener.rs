use std::mem::replace;
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

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq , Debug)]
pub struct ClientSocketsConfig {
    pub client_tcp_socket_config : Option<SocketConfig>,
    pub client_tls_socket_config : Option<( SocketConfig, TlsAcceptorConfig)>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ListenerEvent{
    BuildingSockets,
    SocketsConfigChanged,
    SocketsConfigIs(ClientSocketsConfig),
    SocketsBuilt,
    ReceivedNewConnection(ClientAddr),
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
        let _ = event_sender.send(ListenerEvent::SocketsConfigIs(configs.clone()));

        let _ = event_sender.send(ListenerEvent::BuildingSockets);
        let client_sockets = ClientSockets::build_client_sockets(&configs.client_tcp_socket_config, &configs.client_tls_socket_config).unwrap();
        let _ = event_sender.send(ListenerEvent::SocketsBuilt);
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
                    let (client_stream, client_socket_addr) = accept_res?;
                    self.handle_new_connection(client_stream, client_socket_addr).await.unwrap();
                },
                changed_res = self.config_reader.changed() => {
                    let _ = changed_res?;
                    
                }
            }
        }
    }

    pub fn rebuild_sockets(&mut self, configs:&ClientSocketsConfig) -> Result<(), anyhow::Error>{
        let _ = replace(&mut self.client_sockets, Self::build_sockets(configs, &mut self.event_sender)?);
        Ok(())
    }

    pub fn build_sockets(configs: &ClientSocketsConfig, event_sender        : &mut BroadcastSender<ListenerEvent>) -> Result<ClientSockets, anyhow::Error> {
        let _ = event_sender.send(ListenerEvent::SocketsConfigIs(configs.clone()));
        let _ = event_sender.send(ListenerEvent::BuildingSockets);

        let client_sockets = ClientSockets::build_client_sockets(&configs.client_tcp_socket_config, &configs.client_tls_socket_config).unwrap();

        let _ = event_sender.send(ListenerEvent::SocketsBuilt);

        Ok(client_sockets)
    }
    
    pub async fn handle_new_connection(&mut self, client_stream: ClientStream, client_addr: SocketAddr) -> Result<(), anyhow::Error> {
        let client_addr = ClientAddr(client_addr.ip());
        let _ = self.event_sender.send(ListenerEvent::ReceivedNewConnection(client_addr.clone()));

        if self.client_permission_list.borrow_and_update().allows(&client_addr){
            self.new_conns_sender.send((client_stream, client_addr)).await?; 
            let _ = self.event_sender.send(ListenerEvent::ConnectionAccepted);

        } else {
            drop(client_stream);
            let _ = self.event_sender.send(ListenerEvent::ConnectionDenied);

        }
        Ok(())
    }
}
