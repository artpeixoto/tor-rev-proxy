pub mod controller;
pub mod host;
pub mod init;
pub mod main_process;
pub mod renames;
pub mod tools;
pub mod types;

use arti_client::{
    config::TorClientConfigBuilder, TorAddr, TorClient, TorClientBuilder, TorClientConfig,
};
use clap::Parser;
use controller::Controller;
use futures::FutureExt;
use host::HostAddrGetter;
use httparse::{self, Status};
use init::{get_init, InitArgs};
use main_process::connection_manager::ConnectionManager;
use renames::{broadcast_channel, watch_channel};
use std::{
    borrow::Cow,
    future::IntoFuture,
    iter::repeat,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Not,
    str::{from_utf8, FromStr},
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    join,
    net::TcpSocket,
    sync::OnceCell,
};
use url::Url;

#[tokio::main()]
async fn main() -> Result<(), anyhow::Error> {
    let init = get_init();
    let host_addr_getter = HostAddrGetter::new();
    let (conn_mngr_cfg, conn_cfg) = init.build_configs();

    let (conn_mngr_cfg_sender, conn_mngr_cfg_receiver) = watch_channel(conn_mngr_cfg);
    let (conn_cfg_sender, conn_cfg_receiver)            = watch_channel(conn_cfg);

    let (conn_evt_sender, conn_evt_receiver)            = broadcast_channel(16 * 1024);
    let (conn_mngr_evt_sender, conn_mngr_evt_receiver)  = broadcast_channel(1024);

    let host_socket = init.build_host_socket();
    let client_socket = init.build_client_socket().listen(1024).unwrap();

    let mut conn_mngr = ConnectionManager::new(
        host_addr_getter,
        conn_mngr_evt_sender,
        conn_evt_sender,
        conn_mngr_cfg_receiver,
        conn_cfg_receiver,
        client_socket,
        host_socket,
    );

    conn_mngr.listen_for_connections().await
}
