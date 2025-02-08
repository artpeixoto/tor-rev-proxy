use std::{sync::Arc, time::SystemTime};

use chrono::{DateTime, Local};
use dashmap::DashMap;

use crate::{
    logger::Event,
    main_process::{
        conn_builder::BuilderEvent,
        connection::{ConnectionConfig, ConnectionController, ConnectionEvent},
        listener::{ClientSocketsConfig, ListenerEvent},
        manager::{ClientsPermissionList, ConnectionKey},
    },
    renames::WatchSender,
    tools::{broadcast_receiver_ext::ReceiverExt, event_channel::EventReceiver},
    types::client_addr::ClientAddr,
};

pub struct Controller {
    events_listeners: EventsListeners,
    configs: Configurations,
    connections: Arc<DashMap<ConnectionKey, ConnectionController>>,
}

pub struct EventsListeners {
    connections: EventReceiver<(ClientAddr, ConnectionEvent)>,
    builder: EventReceiver<BuilderEvent>,
    listener: EventReceiver<ListenerEvent>,
}
impl EventsListeners {
    pub fn read_all(&mut self) -> Result<Vec<(DateTime<Local>, Event)>, anyhow::Error> {
        let mut events = Vec::new();

        events.extend(
            self.builder
            .recv_all()?
            .into_iter()
            .map(|(t, e)| (t, e.into())),
        );

        events.extend(
            self.connections
            .recv_all()?
            .into_iter()
            .map(|(t, e)| (t, e.into())),
        );

        events.extend(
            self.listener
            .recv_all()?
            .into_iter()
            .map(|(t, e)| (t, e.into())),
        );

        events.sort_by_key(|(st, _e)| *st);

        Ok(events)
    }
}
pub struct Configurations {
    client_sockets: WatchSender<ClientSocketsConfig>,
    client_permission_list: WatchSender<ClientsPermissionList>,
    connections: WatchSender<ConnectionConfig>,
}

impl Controller {
    pub fn new(
        client_sockets_config: WatchSender<ClientSocketsConfig>,
        client_permission_list: WatchSender<ClientsPermissionList>,
        connections_config: WatchSender<ConnectionConfig>,

        listener_event_listener: EventReceiver<ListenerEvent>,
        builder_event_listener: EventReceiver<BuilderEvent>,
        connections_event_listener: EventReceiver<(ClientAddr, ConnectionEvent)>,

        connections: Arc<DashMap<ConnectionKey, ConnectionController>>,
    ) -> Self {
        Self {
            configs: Configurations {
                client_sockets: client_sockets_config,
                client_permission_list,
                connections: connections_config,
            },
            events_listeners: EventsListeners {
                connections: connections_event_listener,
                builder: builder_event_listener,
                listener: listener_event_listener,
            },

            connections: connections,
        }
    }
}

pub const CONTROLLER_API_PIPE: &str = r"127.0.0.1:8080";

pub mod controller_api {
    pub mod api {

        use std::time::SystemTime;

        use chrono::{DateTime, Local};
        use http::Method;
        use serde::{de::DeserializeOwned, Serialize};

        use crate::{
            logger::Event,
            main_process::{
                connection::ConnectionConfig, listener::ClientSocketsConfig,
                manager::ClientsPermissionList,
            },
            types::client_addr::ClientAddr,
        };

        pub const API_BASE_ADDR: &str = "/tor-rev-proxy/controller";

        pub trait ApiMethodDefn {
            const METHOD: Method;
            const PATH: &'static str;
            type Arg: Serialize + DeserializeOwned;
            type Ret: DeserializeOwned + Serialize;
        }

        macro_rules! define_api_method{
            ($type_name:ident = $method:ident $path:literal : $arg_ty:ty => $ret_ty:ty) => {
                #[allow(non_camel_case_types)]
                pub struct $type_name;
                impl ApiMethodDefn for $type_name{
                    const METHOD: http::Method = http::Method::$method;
                    const PATH: &'static str = $path;
                    type Arg = $arg_ty;
                    type Ret = $ret_ty;
                }
            };
            {$($type_name:ident = $method:ident $path:literal : $arg_ty:ty => $ret_ty:ty;)* } => {
                $(
                    define_api_method!($type_name = $method $path : $arg_ty => $ret_ty);
                )*
            }
        }

        define_api_method!(
            GET_CLIENT_SOCKETS_CONFIG =
                GET "/config/client-sockets/": () => ClientSocketsConfig;

            GET_PERMISSION_LIST =
                GET "/config/permission-list/" : () => ClientsPermissionList;

            GET_CONNECTIONS_CONFIGS =
                GET "/config/connections/": ()  => ConnectionConfig;

            GET_EVENTS =
                GET "/events/": () => Vec<(DateTime<Local>, Event)>;

            GET_CURRENT_CONNECTIONS =
                GET "/state/connections/" : () => Vec<ClientAddr>;

            SET_PERMISSION_LIST =
                POST "/config/permission-list/" : ClientsPermissionList => ();

        );
    }

    pub mod client {
        use std::{str::FromStr, time::SystemTime};

        use chrono::{DateTime, Local};
        use reqwest::{Client};
        use url::Url;

        use crate::{
            logger::Event,
            main_process::{connection::ConnectionConfig, manager::ClientsPermissionList},
            types::client_addr::ClientAddr,
        };

        use super::api::{
            ApiMethodDefn, API_BASE_ADDR, GET_CONNECTIONS_CONFIGS, GET_CURRENT_CONNECTIONS,
            GET_EVENTS, GET_PERMISSION_LIST,
        };

        pub struct ControllerClient {
            client: Client,
        }

        pub struct ConnectionError;
        impl ControllerClient {
            pub fn new() -> Self{
                Self{client:Client::new()}
            }
            pub async fn get_events(&mut self) -> Result<Vec<(DateTime<Local>, Event)>, anyhow::Error> {
                self.call_method::<GET_EVENTS>(()).await
            }
            pub async fn get_connections_config(
                &mut self,
            ) -> Result<ConnectionConfig, anyhow::Error> {
                self.call_method::<GET_CONNECTIONS_CONFIGS>(()).await
            }
            pub async fn get_current_connections(
                &mut self,
            ) -> Result<Vec<ClientAddr>, anyhow::Error> {
                self.call_method::<GET_CURRENT_CONNECTIONS>(()).await
            }
            pub async fn get_permission_list(
                &mut self,
            ) -> Result<ClientsPermissionList, anyhow::Error> {
                self.call_method::<GET_PERMISSION_LIST>(()).await
            }

            pub async fn call_method<M: ApiMethodDefn>(
                &mut self,
                args: <M as ApiMethodDefn>::Arg,
            ) -> Result<<M as ApiMethodDefn>::Ret, anyhow::Error> {
                let resp = self
                    .client
                    .request(
                        <M as ApiMethodDefn>::METHOD,
                        Url::from_str(&format!("http://127.0.0.1:8080{}{}", API_BASE_ADDR, M::PATH))
                            .unwrap(),
                    )
                    .json(&args).send().await?
                    .error_for_status()?.json().await?;

                Ok(resp)
            }
        }
    }

    pub mod server {
        use std::sync::Arc;

        use axum::{extract::State, routing::get, Json, Router};
        use tokio::{net::TcpListener, sync::RwLock};

        use crate::{main_process::connection::ConnectionConfig, types::client_addr::ClientAddr};

        use super::{
            super::Controller,
            api::{
                ApiMethodDefn, API_BASE_ADDR, GET_CLIENT_SOCKETS_CONFIG, GET_CONNECTIONS_CONFIGS,
                GET_CURRENT_CONNECTIONS, GET_EVENTS, GET_PERMISSION_LIST,
            },
        };

        pub async fn make_server_socket() -> Result<TcpListener, anyhow::Error> {
            use crate::controller::controller::CONTROLLER_API_PIPE;
            Ok(TcpListener::bind(CONTROLLER_API_PIPE).await?)
        }

        impl Controller {
            pub async fn run_api(self) -> Result<(), anyhow::Error> {
                let socket = make_server_socket().await.unwrap();
                let router = make_router(self);

                axum::serve(socket, router).await.unwrap();
                Ok(())
            }
        }

        type ControllerState = State<Arc<RwLock<Controller>>>;

        fn make_router(controller: Controller) -> Router {
            let router = Router::new().nest(
                API_BASE_ADDR,
                Router::new()
                    .route(&GET_CLIENT_SOCKETS_CONFIG::PATH, get(get_sockets_config))
                    .route(&GET_PERMISSION_LIST::PATH, get(get_permission_list))
                    .route(&GET_CONNECTIONS_CONFIGS::PATH, get(get_connections_config))
                    .route(&GET_EVENTS::PATH, get(get_events))
                    .route(&GET_CURRENT_CONNECTIONS::PATH, get(get_current_connections))
                    .with_state(Arc::new(RwLock::new(controller))),
            );

            router
        }

        pub async fn get_sockets_config(
            state: State<Arc<RwLock<Controller>>>,
        ) -> Json<<GET_CLIENT_SOCKETS_CONFIG as ApiMethodDefn>::Ret> {
            Json(state.read().await.configs.client_sockets.borrow().clone())
        }

        pub async fn get_permission_list(
            state: State<Arc<RwLock<Controller>>>,
        ) -> Json<<GET_PERMISSION_LIST as ApiMethodDefn>::Ret> {
            Json(
                state
                    .read()
                    .await
                    .configs
                    .client_permission_list
                    .borrow()
                    .clone(),
            )
        }

        pub async fn get_connections_config(
            state: State<Arc<RwLock<Controller>>>,
        ) -> Json<ConnectionConfig> {
            Json(state.read().await.configs.connections.borrow().clone())
        }

        pub async fn get_events(
            state: State<Arc<RwLock<Controller>>>,
        ) -> Json<<GET_EVENTS as ApiMethodDefn>::Ret> {
            Json(state.write().await.events_listeners.read_all().unwrap())
        }

        pub async fn get_current_connections(state: ControllerState) -> Json<Vec<ClientAddr>> {
            Json(
                state
                    .read()
                    .await
                    .connections
                    .iter()
                    .map(|i| i.key().clone())
                    .collect(),
            )
        }
    }
}
