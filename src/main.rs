pub mod host;
pub mod proxy;
pub mod main_process;
pub mod controller;
pub mod safe_vec;
pub mod init;

use controller::Controller;
use futures::FutureExt;
use httparse::{self , Status};
use init::{get_init, InitArgs}; 
use std::{ borrow::Cow, future::IntoFuture, iter::repeat, net::{IpAddr, Ipv4Addr, SocketAddr}, ops::Not, str::{from_utf8, FromStr}, sync::Arc};
use arti_client::{TorAddr, TorClient};
use clap::Parser;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, join, net::TcpSocket, sync::OnceCell};
use url::Url;

#[tokio::main()]
async fn main() -> Result<(), anyhow::Error> {
    let init = get_init();

    let controller = Controller::new();
    let controller_api_interface
    
    start_serving(init).await?;

    let tcp_socket = TcpSocket::new_v4()?;

    let tor_host_full_addr   = Arc::new(TorAddr::from((&init.host as &str, init.host_port))?);
    let tor_host_addr       = Arc::new(init.host.clone());

    let tor_client = 
        TorClient::builder()
        .bootstrap_behavior(arti_client::BootstrapBehavior::OnDemand)
        .create_bootstrapped()
        .await?;
    
    tcp_socket.bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), init.serve_port))?;

    let tcp_listener = tcp_socket.listen(1024)?;
    
    loop{
        let Ok((mut tcp_stream, client_addr)) = tcp_listener.accept().await else {continue};
        println!("initialized connection with {client_addr}");

        let stream_task =  {
            // locals
            let tor_host_full_addr  = tor_host_full_addr.clone();
            let tor_host_addr       = tor_host_addr.clone();
            let tor_client          = tor_client.clone();
            
            // task itself
            Box::pin( async move {
                let this_addr = Arc::new(OnceCell::new());
                
                let mut tor_stream = tor_client.connect(tor_host_full_addr.as_ref()).await?;
                tor_stream.wait_for_connection().await.unwrap();
                let (tor_stream_reader, tor_stream_writer) = tor_stream.split();
                let (tcp_stream_reader, tcp_stream_writer) = tcp_stream.split();


                let mut client_to_tor = ClientToTor{
                    this_addr       : this_addr.clone(),
                    tor_addr        : tor_host_addr.clone(),

                    client_reader   : tcp_stream_reader,
                    tor_writer      : tor_stream_writer,
                };
                let mut tor_to_client = TorToClient{
                    this_addr       : this_addr.clone(),
                    tor_addr        : tor_host_addr.clone(),
                    client_writer   : tcp_stream_writer,
                    tor_reader      : tor_stream_reader,
                };
                let client_to_tor_task = client_to_tor.run_task().into_future().map(|x| println!("client_to_tor closed."));
                let tor_to_client_task = tor_to_client.run_task().into_future().map(|x| println!("tor_to_client closed."));

                let _ = join!(client_to_tor_task, tor_to_client_task);

                Result::<(), anyhow::Error>::Ok(())
            } )
        };

        tokio::task::spawn(stream_task);
    }
}
