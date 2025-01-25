use std::net::SocketAddr;
use std::{ops::Not, sync::Arc};

use super::connection::{
    Connection, ConnectionCfgs, ConnectionController, ConnectionEvt, ConnectionInit,
};
use crate::renames::*;
use crate::{
    host::HostAddrGetter,
    tools::permission_list::PermissionList,
    types::client_addr::{self, ClientAddr},
};
use anyhow::{anyhow, Error};
use arti_client::TorClient;
use async_stream::try_stream;
use dashmap::DashMap;
use futures::{future::join, FutureExt};
use tokio::{
    net::TcpListener,
    task::{spawn_local, JoinHandle},
};
use tor_rtcompat::tokio::PreferredRuntime;
use uuid::Uuid;

#[derive(Clone)]
pub struct ConnectionManagerCfgs {
    pub permission_list: PermissionList<ClientAddr>,
}

pub struct ConnectionManager {
    host_addr_getter	: HostAddrGetter,

    conn_mgr_evt_sender	: BroadcastSender<ConnectionManagerEvt>,
    conn_evt_sender		: BroadcastSender<(ClientAddr, ConnectionEvt)>,

    conn_mgr_cfg		: WatchReceiver<ConnectionManagerCfgs>,
    conn_cfg			: WatchReceiver<ConnectionCfgs>,

    conns				: Arc<DashMap<ClientAddr, ConnectionController>>,

    client_socket		: TcpListener,
    host_socket			: TorClient<PreferredRuntime>,
}

impl ConnectionManager {
    pub fn new(
        host_addr_getter	: HostAddrGetter,
        conn_mgr_evt_sender	: BroadcastSender<ConnectionManagerEvt>,
        conn_evt_sender		: BroadcastSender<(ClientAddr, ConnectionEvt)>,
        conn_mgr_cfg		: WatchReceiver<ConnectionManagerCfgs>,
        conn_cfg			: WatchReceiver<ConnectionCfgs>,
        client_socket		: TcpListener,
        host_socket			: TorClient<PreferredRuntime>,
    ) -> Self {
        Self {
            host_addr_getter,
            conn_mgr_evt_sender,
            conn_evt_sender,
            conn_mgr_cfg,
            conn_cfg,
            conns: Arc::new(Default::default()),
            client_socket,
            host_socket,
        }
    }

    fn add_connection_and_start_its_tasks(
        mut conn				: ConnectionController,
        conn_join_handle		: JoinHandle<Result<(), anyhow::Error>>,
        global_conn_evt_sender	: BroadcastSender<(ClientAddr, ConnectionEvt)>,
        conns					: Arc<DashMap<ClientAddr, ConnectionController>>,
    ) -> Result<(), anyhow::Error> {
        let forward_evts_to_global_sender_task = Box::pin({
            let client_addr = conn.client().clone();
            let mut evt_rec = conn.subscribe_to_events();
            let global_evt_sender = global_conn_evt_sender.clone();

            async move {
                loop {
                    let evt = evt_rec.recv().await?;

                    global_evt_sender
					.send((client_addr.clone(), evt))
					.map_err(|e| anyhow!("Couldn't forward to global events sender"))?;
                }

                #[allow(unreachable_code)]
                Result::<(), anyhow::Error>::Ok(())
            }
        });

        let clean_conn_on_finished_task = {
            let uuid = conn.client().clone();
            let conns = conns.clone();

            Box::pin(async move {
                let _conn_res = conn_join_handle.await;
                let _ = conns.remove(&uuid);
            })
        };

        conn.add_to_dependent_tasks(tokio::spawn(forward_evts_to_global_sender_task));
        conn.add_to_dependent_tasks(tokio::spawn(clean_conn_on_finished_task));

        conns.insert(conn.client().clone(), conn);

        Ok(())
    }
    pub async fn listen_for_connections(&mut self) -> Result<(), anyhow::Error> {
        'LISTEN_FOR_CONNECTIONS: loop {
            let (client_stream, client_addr) = self.client_socket.accept().await?;
            let client_addr = ClientAddr(client_addr.ip());

            if self
                .conn_mgr_cfg
                .borrow()
                .permission_list
                .allows(&client_addr)
                .not()
            {
                continue 'LISTEN_FOR_CONNECTIONS;
            }
            let conn_evt_sender = self.conn_evt_sender.clone();
            let conns = self.conns.clone();

            let conn_init = ConnectionInit {
				client_addr,
                client_stream,
                host_client: self.host_socket.isolated_client(),
                conn_configs: self.conn_cfg.clone(),
                host_addr_getter: self.host_addr_getter.clone(),
            }
            .build_connection()
            .map(move |a| {
                a.map(move |(conn, conn_join_handle)| {
                    Self::add_connection_and_start_its_tasks(
                        conn,
                        conn_join_handle,
                        conn_evt_sender,
                        conns,
                    )
                })
            });

            tokio::spawn(conn_init);
        }
    }
}

#[derive(Clone)]
pub enum ConnectionManagerEvt {
    ReceivedConnReq,
    ConnectionEstabilished,
    ConnectionDenied,
}
