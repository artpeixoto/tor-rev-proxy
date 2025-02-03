use std::{net::IpAddr, sync::Arc};

use dashmap::DashMap;

use crate::{
    main_process::{
        conn_builder::BuilderEvent, connection::{ConnectionConfig, ConnectionController, ConnectionEvent, ConnectionTaskHandle}, listener::{ClientSocketsConfig, Listener, ListenerEvent}, manager::{ClientsPermissionList, ConnectionKey}
    }, 
    renames::{BroadcastReceiver, WatchSender}, 
    types::client_addr::ClientAddr
};
pub struct Controller {
    events_listeners: EventsListeners,
    configs         : Configurations,
    connections     : Arc<DashMap<ConnectionKey, ConnectionController>>
}

pub struct EventsListeners{
    connections  : BroadcastReceiver<(ClientAddr, ConnectionEvent)>,
    builder      : BroadcastReceiver<BuilderEvent>,
    listener     : BroadcastReceiver<ListenerEvent>, 
}
pub struct Configurations{
    client_sockets              : WatchSender<ClientSocketsConfig>,
    client_permission_list     : WatchSender<ClientsPermissionList>,
    connections                 : WatchSender<ConnectionConfig>,
}


impl Controller {
    pub fn new( 
        client_sockets_config       : WatchSender<ClientSocketsConfig>,
        client_permission_list      : WatchSender<ClientsPermissionList>,
        connections_config          : WatchSender<ConnectionConfig>,

        listener_event_listener     : BroadcastReceiver<ListenerEvent>,
        builder_event_listener      : BroadcastReceiver<BuilderEvent>,
        connections_event_listener  : BroadcastReceiver<(ClientAddr, ConnectionEvent)>,

        connections: Arc<DashMap<ConnectionKey, ConnectionController>>
    ) -> Self {
        Self {
            configs: Configurations{
                client_sockets: client_sockets_config,
                client_permission_list,
                connections: connections_config,
            },
            events_listeners: EventsListeners{
                connections       : connections_event_listener,
                builder: builder_event_listener,
                listener: listener_event_listener,
            },

            connections: connections
        }
    }

}



pub trait BroadcastReceiverExt{
    type T;
    fn recv_all(&mut self) -> Result<Vec<Self::T>, anyhow::Error>;
}

impl<T: Clone> BroadcastReceiverExt for BroadcastReceiver<T>{
    type T = T; 
    fn recv_all(&mut self) -> Result<Vec<Self::T>, anyhow::Error>{
        use tokio::sync::broadcast::error::TryRecvError;
        let mut vec = Vec::new();

        'GET_EVENTS_LOOP: loop {
            match self.try_recv(){
                Ok(val) =>{
                    vec.push(val);
                    continue 'GET_EVENTS_LOOP;
                },
                Err(TryRecvError::Lagged(_)) => {continue 'GET_EVENTS_LOOP;}
                Err(TryRecvError::Closed)    => {break 'GET_EVENTS_LOOP Err(anyhow::anyhow!("Channel closed"));},
                Err(TryRecvError::Empty)     => {break 'GET_EVENTS_LOOP Ok(vec);}
            }
        }
    }
}
pub mod controller_api{
    use std::sync::Arc;
    pub const CONTROLLER_PIPE_PATH: &str = r"\\.\pipe\tor-rev-proxy-controller";

    use axum::{extract::State, routing::get, Json, Router};
    use tokio::{net::TcpListener, sync::RwLock};

    use crate::{controller::{controller::BroadcastReceiverExt, local_socket::{self, LocalClientSocket, LocalServerSocket}}, main_process::{conn_builder::BuilderEvent, connection::ConnectionEvent, listener::{ClientSocketsConfig, ListenerEvent}, manager::ClientsPermissionList}, types::client_addr::ClientAddr};

    use super::Controller;

    #[cfg(target_os="windows")]
    pub async fn make_server_socket() -> Result<LocalServerSocket, anyhow::Error>{
        use std::{path::PathBuf, str::FromStr};

        Ok(LocalServerSocket::new(
            CONTROLLER_PIPE_PATH.to_string()
        )?)
    }

    pub async fn make_client_socket() -> LocalClientSocket{
        todo!()
    }

    impl Controller{
        pub async fn run_api(self) -> Result<(), anyhow::Error>{
            let socket = make_server_socket().await.unwrap();
            let router = make_router(self);

            axum::serve(socket, router).await.unwrap();
            Ok(())
        }
    }

    fn make_router(controller: Controller) -> Router{
        let router = 
            Router::new()
            .route("/configs/client_sockets" , get(get_sockets_config))
            .route("/configs/permission_list", get(get_permission_list))
            .route("/configs/connections"    , get(get_connections_config))
            .route("/events/connections"     , get(get_connections_events))
            .route("/events/listener"        , get(get_listener_events))
            .route("/events/manager"         , get(get_builder_events))
            .route("/state/connections"      , get(get_current_connections))
            .with_state((Arc::new(RwLock::new(controller))));

        router
    }


    pub async fn get_sockets_config(state: State<Arc<RwLock<Controller>>>) -> Json<ClientSocketsConfig>{
        Json(state.read().await.configs.client_sockets.borrow().clone())
    }

    pub async fn get_permission_list(state: State<Arc<RwLock<Controller>>>) -> Json<ClientsPermissionList>{
        Json(state.read().await.configs.client_permission_list.borrow().clone())
    }
    
    pub async fn get_connections_config(state: State<Arc<RwLock<Controller>>>) -> Json<ClientSocketsConfig>{
        Json(state.read().await.configs.client_sockets.borrow().clone())
    }

    pub async fn get_connections_events(state: State<Arc<RwLock<Controller>>>) -> Json<Vec<(ClientAddr, ConnectionEvent)>> {
        Json(state.write().await.events_listeners.connections.recv_all().unwrap_or(Vec::new()))
    }

    pub async fn get_listener_events(state: State<Arc<RwLock<Controller>>>) -> Json<Vec<ListenerEvent>> {
        Json(state.write().await.events_listeners.listener.recv_all().unwrap_or(Vec::new()))
    }
    pub async fn get_builder_events(state: State<Arc<RwLock<Controller>>>) -> Json<Vec<BuilderEvent>> { 
        Json(state.write().await.events_listeners.builder.recv_all().unwrap_or(Vec::new()))
    }
    pub async fn get_current_connections(state: State<Arc<RwLock<Controller>>>) -> Json<Vec<ClientAddr>>{
        Json(state.read().await.connections.iter().map(|c| c.key().to_owned()).collect::<Vec<_>>())
    }

    // pub async fn get_connections_clients(state: State<Arc<RwLock<Controller>>>) -> Json<Vec<>>{

    // }
}
