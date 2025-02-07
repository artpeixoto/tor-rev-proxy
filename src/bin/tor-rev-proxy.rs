use arti_client::{
    config::TorClientConfigBuilder, TorAddr, TorClient, TorClientBuilder, TorClientConfig,
};
use clap::Parser;
use futures::FutureExt;
use log::{debug, error};
use tor_rev_proxy::{
    controller::controller::Controller, host::HostAddrGetter, init::get_init, logger::EventLogger, main_process::{conn_builder::ConnectionBuilder, listener::Listener, manager::Manager}, renames::{broadcast_channel, mpsc_channel, watch_channel}
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    join,
    net::TcpSocket,
    sync::{watch, OnceCell},
};
use url::Url;

#[tokio::main()]
async fn main() -> Result<(), anyhow::Error> {
    pretty_env_logger::init();
    // console_subscriber::init();

    let init = get_init();
    let host_addr_getter = HostAddrGetter::new();
    let (conn_config_writer, conn_config_reader) = watch_channel(init.conn_cfgs);
    let (permission_list_writer, permission_list_reader) = watch_channel(init.permission_list);
    let (sockets_config_writer, sockets_config_reader) = watch_channel(init.client_sockets);

    let (builder_events_sender, builder_events_receiver)    = broadcast_channel(1024);
    let (listener_events_sender, listener_events_receiver)  = broadcast_channel(1024);

    let (conn_events_sender, conn_events_receiver)          = broadcast_channel(2048);
    let (connections_sender, connections_receiver)          = mpsc_channel(256);


    let mut evt_logger = EventLogger::new(
        &conn_events_receiver, 
        &builder_events_receiver, 
        &listener_events_receiver
    );

    let connection_builder =
        ConnectionBuilder::new(
            builder_events_sender,
            host_addr_getter, 
            conn_config_reader, 
            conn_events_sender,
        )
        .await
        .unwrap();

    let mut connections_manager = 
        Manager::new(
            connection_builder,
            connections_receiver,
            permission_list_reader.clone(),
        );

    let connections = connections_manager.get_connections();

    let mut connections_listener = 
        Listener::new(
            sockets_config_reader,
            listener_events_sender,
            connections_sender,
            permission_list_reader
        )
        .unwrap();

    let controller = Controller::new(
        sockets_config_writer,
        permission_list_writer,
        conn_config_writer,
        listener_events_receiver,
        builder_events_receiver,
        conn_events_receiver,
        connections,
    );
    
    debug!("Spawning event logger");
    tokio::spawn(async move{ evt_logger.run().await });
    tokio::spawn(async move{ controller.run_api().await.unwrap() });
    tokio::spawn(async move{ connections_manager.run().await.unwrap() });
    connections_listener.run().await?;
    Ok(())
}
