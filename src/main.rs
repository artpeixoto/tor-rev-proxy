use futures::FutureExt;
use httparse::{self , Header, Status}; 
use std::{borrow::Cow, future::IntoFuture, iter::repeat, net::{IpAddr, Ipv4Addr, SocketAddr}, ops::Not, str::{from_utf8, FromStr}, sync::Arc};
use arti_client::{TorAddr, TorClient};
use clap::Parser;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, join, net::TcpSocket, sync::OnceCell};
use url::Url;

#[tokio::main()]

async fn main() -> Result<(), anyhow::Error> {
    let init = Init::try_parse()?;
    start_serving(&init).await?;
    Ok(())
}

// Simple onion to tcp service router
#[derive(clap::Parser)]
struct Init{
    // Port that will receive the requests
    #[arg(short='p', long="serve-port", default_value_t=80) ]    
    serve_port  : u16,

    // The tor host to which the requests will be forwarded to 
    #[arg(short='H', long="host" , default_value_t=("haystak5njsmn2hqkewecpaxetahtwhsbsa64jom2k22z5afxhnpxfid.onion".to_string()))]
        host        : String,

    #[arg(long="host-port", short='P', default_value_t=80)]
    host_port   : u16
}


async fn start_serving(init: &Init) -> Result<(), anyhow::Error>{
    let mut tcp_socket = TcpSocket::new_v4()?;

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
    Ok(())
}

use tokio::net::tcp::WriteHalf as TcpWriter;
use tokio::net::tcp::ReadHalf as TcpReader;

use arti_client::DataReader as TorReader;
use arti_client::DataWriter as TorWriter;


struct TorToClient<'a>{
    this_addr       : Arc<OnceCell<String>>,

    tor_addr        : Arc<String>,

    client_writer   : TcpWriter<'a>,
    tor_reader      : TorReader,
}

impl<'a> TorToClient<'a> { 
    async fn run_task(&mut self) -> Result<(), anyhow::Error>{
        let mut buf = [0_u8; 64*1024];
        loop{
            let Ok(read) = self.tor_reader.read(&mut buf).await else {break};
            let read_bytes = &buf[0..read];
            
            self.client_writer.write_all(read_bytes).await?;
        }
        Result::<(), anyhow::Error>::Ok(())
    }
}

struct ClientToTor<'a>{
    this_addr       : Arc<OnceCell<String>>,

    tor_addr        : Arc<String>,
    // tor_port        : u16,  

    client_reader   : TcpReader<'a>,
    tor_writer      : TorWriter,
}


impl<'a> ClientToTor<'a>{
    pub async fn run_task(&mut self) -> Result<(), anyhow::Error>{
        'TASK_LOOP: loop{

            let mut in_req_buf      = vec![0_u8; 8*1024];
            let mut read            = 0;  //keeps track of the amount read

            'READ_AND_TRANSLATE: loop{
                // wait for message
                let Ok(just_read) = self.client_reader.read(&mut in_req_buf[read..]).await else {
                    //something happened to the connection. We just stop doing anything then
                    break 'TASK_LOOP;
                };
                
                // advances amount read
                read = read + just_read; 

                if just_read == 0 && read == 0{
                    break 'TASK_LOOP;
                }

                println!("Received message from client {read}");
                let in_req_bytes = &in_req_buf[0..read];

                // first, we try to manipulate the content as http. 
                let mut headers_buf = [httparse::EMPTY_HEADER; 128];
                let mut in_req      = httparse::Request::new(&mut headers_buf);

                match in_req.parse(in_req_bytes){
                    Ok(Status::Complete(body_start)) => {

                        // write first line
                        if let Some(m) = in_req.method.as_ref(){                            
                            self.tor_writer.write_all(m.as_bytes()).await.unwrap();
                            self.tor_writer.write_all(b" ").await.unwrap();
                        }

                        if let Some(p) = in_req.path.as_ref(){
                            self.tor_writer.write_all(p.as_bytes()).await.unwrap();
                            self.tor_writer.write_all(b" ").await.unwrap();
                        };

                        self.tor_writer.write_all(b"HTTP/1.1\r\n").await.unwrap();                        

                        //write headers
                        for header in in_req.headers{
                            let header_value = match header.name{
                                "Host" => {
                                    if self.this_addr.initialized().not(){
                                        let header_value = from_utf8(header.value).unwrap().to_string();
                                        self.this_addr.set(header_value).unwrap();
                                    }

                                    Cow::Borrowed(self.tor_addr.as_str().as_bytes())
                                },
                                "Referer" => {
                                    let mut url = Url::from_str(from_utf8(header.value).unwrap()).unwrap();

                                    url.set_host(Some(self.tor_addr.as_str())).unwrap();

                                    Cow::Owned(url.to_string().into_bytes())
                                }
                                _ => Cow::Borrowed(header.value)
                            };
                            self.tor_writer.write_all(header.name.as_bytes()).await.unwrap();
                            self.tor_writer.write_all(b": ").await.unwrap();
                            self.tor_writer.write_all(&header_value).await.unwrap();
                            self.tor_writer.write_all(b"\r\n").await.unwrap();
                        }
                        self.tor_writer.write_all(b"\r\n").await.unwrap();

                        let body_bytes = &in_req_bytes[body_start..];
                        self.tor_writer.write_all(body_bytes).await.unwrap();
                        self.tor_writer.flush().await.unwrap();
                        continue 'TASK_LOOP;
                    },
                    Ok(Status::Partial)            => {
                        // incomplete, lets read more. 
                        in_req_buf.extend((0..(8*1024)).into_iter().map(|_| 0_u8));
                        continue 'READ_AND_TRANSLATE;
                    },
                    Err(_e)                         => {
                        // parsing failed. In this case, there is no need to do anything. We just forward the message as is
                        self.tor_writer.write_all(in_req_bytes).await?;
                        self.tor_writer.flush().await.unwrap();
                        continue 'TASK_LOOP;
                    },
                };
            }
        }

        Result::<(), anyhow::Error>::Ok(())
    }
}
