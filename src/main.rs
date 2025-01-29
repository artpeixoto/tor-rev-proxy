pub mod controller;
pub mod host;
pub mod init;
pub mod main_process;
pub mod renames;
pub mod tools;
pub mod types;
pub mod comm_channels;

use arti_client::{
    config::TorClientConfigBuilder, TorAddr, TorClient, TorClientBuilder, TorClientConfig,
};
use clap::Parser;
use controller::controller::Controller;
use futures::FutureExt;
use host::HostAddrGetter;
use httparse::{self, Status};
use init::{get_init, InitArgs};
use main_process::{process::Process};
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
    let (process_endpoint, controller_endpoint) = comm_channels::make_comm_channels(init.build_configs());

    let process_controller = Controller::new(controller_endpoint);

    let mut main_process = Process::new(
        host_addr_getter,
        process_endpoint
    );

    main_process.listen_for_connections().await
}
