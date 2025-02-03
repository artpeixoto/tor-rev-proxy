use std::{ops::Not, sync::Arc};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::{select, sync::watch::Receiver};

use crate::{renames::{BroadcastSender, MpscReceiver, WatchReceiver}, tools::permission_list::{self, PermissionList}, types::client_addr::ClientAddr};

use super::{conn_builder::ConnectionBuilder, connection::ConnectionController, sockets::client_sockets::ClientStream};

pub type ConnectionKey = ClientAddr;

pub type ClientsPermissionList =  PermissionList<ClientAddr>;



pub struct Manager{
	new_conn_input		    :  MpscReceiver<(ClientStream, ClientAddr)>,
    connection_builder	    :  ConnectionBuilder,
    connections			    :  Arc<DashMap<ConnectionKey, ConnectionController>>,

    client_permission_list  : WatchReceiver<ClientsPermissionList>,
}

impl Manager{
    pub fn new(
        builder                 : ConnectionBuilder,
        connections_receiver    : MpscReceiver<(ClientStream, ClientAddr)>,
        permission_list_reader  : WatchReceiver<ClientsPermissionList>,
    ) -> Self{
        Self { 
            new_conn_input: connections_receiver, 
            client_permission_list: permission_list_reader, 
            connection_builder: builder, 
            connections:  Arc::new(DashMap::new()),
        }
    }
    pub async fn run(&mut self) -> Result<(), anyhow::Error>{
        loop{
            select! {
                config_reader_change = self.client_permission_list.changed() => {
                    let _ = config_reader_change?;
                    self.handle_config_changed();
                },

                conn_input = self.new_conn_input.recv() => {
                    let conn = conn_input.ok_or(anyhow::anyhow!("Channel closed"))?;
                    self.add_connection_and_start_its_tasks(conn).await?;
                }
            }        
        }
    }

    pub fn get_connections(&self) -> Arc<DashMap<ConnectionKey, ConnectionController>>{
        self.connections.clone()
    }

    fn handle_config_changed(&mut self) {
        let permission_list = self.client_permission_list.borrow();
        self.connections.retain(|client_addr, _c|{
            permission_list.allows(client_addr)
        })
    }

    async fn add_connection_and_start_its_tasks(
        &mut self, 
        (client_stream, client_addr): (ClientStream,ClientAddr),
    ) -> Result<(), anyhow::Error> {
        if self.client_permission_list.borrow().allows(&client_addr).not(){

        }
        let (mut conn_ctrl, conn_init) = self.connection_builder.build_connection(client_stream, client_addr).await?;

        let conn_task_handle = conn_init.run();

        let clean_conn_on_finished_task = {
            let uuid = conn_ctrl.client_addr().clone();
            let connections = self.connections.clone();

            Box::pin(async move {
                let _conn_res = conn_task_handle.await;
                let _ = connections.remove(&uuid);
                Ok(())
            })
        };
		
        conn_ctrl.add_to_dependent_tasks(tokio::spawn(clean_conn_on_finished_task));
        self.connections.insert(conn_ctrl.client_addr().clone(), conn_ctrl);

        Ok(())
    }
}