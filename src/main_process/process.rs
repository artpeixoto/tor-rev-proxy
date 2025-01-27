use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::{ sync::Arc};

use super::connection::{
    Connection, ConnectionConfig, ConnectionController, ConnectionEvent, ConnectionInit,
};
use crate::comm_channels::{ConfigReaders, EventSenders, ProcessEndpoint};
use crate::init::Init;
use crate::{init, renames::*};
use crate::{
    host::HostAddrGetter,
    tools::permission_list::PermissionList,
    types::client_addr::{self, ClientAddr},
};
use anyhow::{anyhow, Error};
use arti_client::{BootstrapBehavior, TorClient, TorClientConfig};
use async_stream::try_stream;
use dashmap::DashMap;
use futures::future::select_all;
use futures::{future::join, FutureExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpSocket;
use tokio::{
    net::TcpListener,
    task::{spawn_local, JoinHandle},
};
use tor_rtcompat::tokio::PreferredRuntime;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    pub permission_list          : PermissionList<ClientAddr>,
    pub max_simultaneous_clients : u32,
}

pub struct Process {
    host_addr_getter	: HostAddrGetter,

    cfg_readers         : ConfigReaders,
    evt_senders         : EventSenders,
    
    conns				: Arc<DashMap<ClientAddr, ConnectionController>>,

    client_sockets		: Vec<(TcpListener, u16)>,
    host_socket			: TorSocket,
}


impl Process {
    pub fn build_host_socket() -> Result<TorSocket, anyhow::Error>{
		let tor_client = 
            TorClient::builder()
                .config(TorClientConfig::builder().build()?)
                .bootstrap_behavior(BootstrapBehavior::OnDemand)
                .create_unbootstrapped()?;

        Ok(tor_client)
    }

    pub fn build_client_sockets(config: &ProcessConfig) -> Result<Vec<(TcpListener, u16)>, anyhow::Error> {
		let socket_ports = vec![80, 413];

        let sockets = 
            socket_ports
            .into_iter()
            .map(|port| {
                let socket = TcpSocket::new_v4()?;
                socket.bind(
                    SocketAddr::V4( SocketAddrV4::new(
                        Ipv4Addr::UNSPECIFIED, port 
                    ) )
                )? ;
                Ok((socket.listen(config.max_simultaneous_clients)?, port))
            })
            .collect::<Result<Vec<(TcpListener, u16)>, anyhow::Error>>()?;
        
		Ok(sockets)
    }
    pub fn new(
        host_addr_getter	: HostAddrGetter,
        comm_channels       : ProcessEndpoint, 
    ) -> Result<Self, anyhow::Error> {
        let process_config = comm_channels.cfg_readers.process.borrow();
        let client_sockets = Self::build_client_sockets(&process_config)?;
        drop(process_config);

        let host_socket    = Self::build_host_socket()?; 

        Ok(Self {
            host_addr_getter,
            cfg_readers: comm_channels.cfg_readers,
            evt_senders: comm_channels.evt_senders,
            conns: Arc::new(Default::default()),
            client_sockets,
            host_socket,
        })
    }

    fn add_connection_and_start_its_tasks(
        mut conn				: ConnectionController,
        conn_join_handle		: JoinHandle<Result<(), anyhow::Error>>,
        global_conn_evt_sender	: BroadcastSender<(ClientAddr, ConnectionEvent)>,
        conns					: Arc<DashMap<ClientAddr, ConnectionController>>,
    ) -> Result<(), anyhow::Error> {
        let forward_evts_to_global_sender_task = Box::pin({
            let client_addr = conn.client_addr().clone();
            let mut evt_rec = conn.subscribe_to_events();
            let global_evt_sender = global_conn_evt_sender.clone();

            async move {
                loop {
                    let evt = evt_rec.recv().await?;

                    global_evt_sender
					.send((client_addr.clone(), evt))
					.map_err(|_e| anyhow!("Couldn't forward to global events sender"))?;
                }

                #[allow(unreachable_code)]
                Result::<(), anyhow::Error>::Ok(())
            }
        });

        let clean_conn_on_finished_task = {
            let uuid = conn.client_addr().clone();
            let conns = conns.clone();

            Box::pin(async move {
                let _conn_res = conn_join_handle.await;
                let _ = conns.remove(&uuid);
            })
        };

        conn.add_to_dependent_tasks(tokio::spawn(forward_evts_to_global_sender_task));
        conn.add_to_dependent_tasks(tokio::spawn(clean_conn_on_finished_task));

        conns.insert(conn.client_addr().clone(), conn);

        Ok(())
    }

    pub async fn listen_for_connections(&mut self) -> Result<(), anyhow::Error> {
        'LISTEN_FOR_CONNECTIONS: loop {
            let ((client_stream, client_addr), input_port) = 
                select_all(
                    self
                    .client_sockets
                    .iter_mut()
                    .map(|(socket, socket_port)| { Box::pin( async {
                        let res : Result<_, anyhow::Error> = Ok((socket.accept().await?, *socket_port ));
                        res
                    }) })
                )
                .await
                .0?;

            let client_addr = ClientAddr(client_addr.ip());

            let is_client_addr_allowed = self.cfg_readers.process.borrow().permission_list.allows(&client_addr);

            if !is_client_addr_allowed {
                let _  = self.evt_senders.process.send(ProcessEvent::ConnectionDenied);
                continue 'LISTEN_FOR_CONNECTIONS;
            }

            let conn_evt_sender = self.evt_senders.conn.clone();
            let conns = self.conns.clone();
            let conn_init = 
                ConnectionInit {
                    client_addr,
                    client_stream,
                    host_client     : self.host_socket.isolated_client(),
                    conn_configs    : self.cfg_readers.conn.clone(),
                    host_addr_getter: self.host_addr_getter.clone(),
                    input_port
                }
                .build_connection()
                .map( move |a| a.map( move |(conn, conn_join_handle)| 
                    Self::add_connection_and_start_its_tasks(
                        conn,
                        conn_join_handle,
                        conn_evt_sender,
                        conns,
                    )
                ));

            tokio::spawn(conn_init);
        }
    }
}

#[derive(Clone)]
pub enum ProcessEvent {
    ReceivedConnection,
    ConnectionEstabilished,
    ConnectionDenied,
}
