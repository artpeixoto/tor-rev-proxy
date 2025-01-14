use http::Uri;
use httparse::{self , Header, Status}; 
use regex::Regex;
use std::{borrow::Cow, future::IntoFuture, io::{BufRead, Bytes}, net::{IpAddr, Ipv4Addr, SocketAddr}, ops::Not, result, str::{from_utf8, FromStr}, sync::{Arc, RwLock}};
use anyhow::anyhow;
use arti_client::{TorAddr, TorClient, TorClientConfig};
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

    let tor_host_full_addr       = Arc::new(TorAddr::from((&init.host as &str, init.host_port))?);
    let tor_host_addr  = Arc::new(init.host.clone());

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
                let this_addr = Arc::new(RwLock::new(Option::<String>::None));
                
                let mut tor_stream = tor_client.connect(tor_host_full_addr.as_ref()).await?;
                tor_stream.wait_for_connection().await.unwrap();
                let (mut tor_stream_reader, mut tor_stream_writer) = tor_stream.split();
                let (mut tcp_stream_reader, mut tcp_stream_writer) = tcp_stream.split();

                let client_to_tor_task = {
                    // locals  
                    let this_addr           = this_addr.clone();
                    let tor_host_full_addr  = tor_host_full_addr.clone();
                    let tor_host_addr       = tor_host_addr.clone();

                    // client_to_tor task
                    Box::pin(async move{
                        let mut buf = [0_u8; 8192];
                        let mut has_set_this_addr = false;

                        // we remove the port information

                        loop{
                            let Ok(read) = tcp_stream_reader.read(&mut buf).await else {break};

                            // println!("sending from client to host");

                            let read_bytes              = &buf[0..read];

                            // tor_stream_writer.write_all(read_bytes).await.unwrap();
                            let mut has_changed_host    = false;
                            let mut has_changed_referer = false;
                            let mut out_req = Vec::new();

                            for line in read_bytes.split(|x| x == &b'\n'){
                                let line = line.trim_ascii_end();
                                const HOST_STR:&[u8] = b"Host: "; 
                                if !has_changed_host && line.starts_with(HOST_STR){
                                    if !has_set_this_addr{
                                        let this_addr_val = from_utf8(&line[HOST_STR.len()..]).unwrap().to_string();
                                        println!("i am '{this_addr_val}'");
                                        *this_addr.write().unwrap() = Some(this_addr_val);
                                        has_set_this_addr = true;
                                    }

                                    // out_req.extend_from_slice(b"127.0.0.1");
                                    out_req.extend_from_slice(HOST_STR);
                                    out_req.extend_from_slice(tor_host_addr.as_bytes());
                                    has_changed_host = true;
                                } else {
                                    out_req.extend_from_slice(line);
                                }
                                out_req.extend_from_slice(b"\r\n");
                            }

                            let out_req_str = from_utf8(&out_req).unwrap();
                            if out_req_str.chars().all(|c| char::is_whitespace(c)).not(){
                                let read_bytes_str = from_utf8(read_bytes).unwrap();
                                println!("income: {read_bytes_str:?}");
                                println!("sending from client to host: '{out_req_str:?}'");
                                let out_req_is_eq =  &out_req as &[u8] == read_bytes; 

                                println!("out_req_is_eq: {out_req_is_eq}");                                
                            }


                            tor_stream_writer.write_all(&out_req).await.unwrap();
                            tor_stream_writer.flush().await.unwrap();
                        }
                        Result::<(), anyhow::Error>::Ok(())
                    })
                };

                let tor_to_client_task = Box::pin( async move {
                });

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
    this_addr       : &'a OnceCell<String>,

    tor_addr        : &'a Arc<TorAddr>,
    tor_port        : u16,  

    client_writer   : TcpWriter<'a>,
    tor_reader      : TorReader,
}

impl<'a> TorToClient<'a> { 
    async fn run_task(&mut self) -> Result<(), anyhow::Error>{
        let mut buf = [0_u8; 64*1024*1024];
        loop{
            let Ok(read) = self.tor_reader.read(&mut buf).await else {break};
            println!("sending from host to client....");
            let read_bytes = &buf[0..read];
                
            'PRINT_BYTES: {// try to print bytes
                let Ok(read_bytes_str) =  from_utf8( read_bytes) else {break 'PRINT_BYTES};
                println!("to client =======\n {read_bytes_str} \n============");
            }
            self.client_writer.write_all(read_bytes).await?;
        }
        Result::<(), anyhow::Error>::Ok(())
    }
}

struct ClientToTor<'a>{
    this_addr       : &'a OnceCell<String>,

    tor_addr        : &'a Arc<TorAddr>,
    tor_port        : u16,  

    client_reader   : TcpReader<'a>,
    tor_writer      : TorWriter,
}


impl<'a> ClientToTor<'a>{
    pub async fn run_task(&mut self) -> Result<(), anyhow::Error>{
        'TASK_LOOP: loop{
            let mut in_req_buf      = Vec::with_capacity(8192);
            let mut read            = 0;  //keeps track of the amount read
            let mut out_req         = Vec::with_capacity(8192);

            'READ_AND_TRANSLATE: loop{
                // wait for message
                let Ok(just_read) = self.client_reader.read(&mut in_req_buf[read..]).await else {
                    //something happened to the connection. We just stop doing anything then
                    break 'TASK_LOOP;
                };

                // advances amount read
                read = read + just_read; 

                let in_req_bytes = &in_req_buf[0..read];

                // first, we try to manipulate the content as http. 
                let mut headers_buf = [httparse::EMPTY_HEADER; 128];
                let in_req          = httparse::Request::new(&mut headers_buf);

                match in_req.parse(in_req_bytes){
                    Ok(Status::Complete(complete)) => {
                        // a http req
                        out_req.push(format!(""))
                    },
                    Ok(Status::Partial)            => {
                        // incomplete. 
                        continue 'READ_AND_TRANSLATE;
                    },
                    Err(e)                         => {
                        // parsing failed. In this case, there is no need to do anything. We just forward the message as is
                        self.tor_writer.write_all(in_req_bytes).await?;
                        self.tor_writer.flush().await;
                        continue 'TASK_LOOP;
                    },
                };
            }




            // tor_stream_writer.write_all(read_bytes).await.unwrap();
            let mut has_changed_host    = false;
            let mut has_changed_referer = false;

            let mut lines = read_bytes.split(|x| *x == b'\n');

            //the first line is the http command. Nothing important here, really 
        

            // lines are split by a "/n/r" 
            // we use /n as an anchor...
                // ...and remove the '\r' at the end
                let line = from_utf8( line)?.strip_suffix("\r").unwrap();

                const HOST_STR:&[u8] = b"Host: "; 
                if !has_changed_host && line.starts_with(HOST_STR){
                    if !self.this_addr.initialized(){
                        let this_addr_val = from_utf8(&line[HOST_STR.len()..]).unwrap().to_string();
                        println!("i am '{this_addr_val}'");
                        self.this_addr.set(this_addr_val);
                    }

                    // out_req.extend_from_slice(b"127.0.0.1");
                    out_req.extend_from_slice(HOST_STR);
                    out_req.extend_from_slice(self.tor_addr.as_bytes());
                    has_changed_host = true;
                } else {
                    out_req.extend_from_slice(line);
                }
                out_req.extend_from_slice(b"\r\n");

            let out_req_str = from_utf8(&out_req).unwrap();
            if out_req_str.chars().all(|c| char::is_whitespace(c)).not(){
                let read_bytes_str = from_utf8(read_bytes).unwrap();
                println!("income: {read_bytes_str:?}");
                println!("sending from client to host: '{out_req_str:?}'");
                let out_req_is_eq =  &out_req as &[u8] == read_bytes; 

                println!("out_req_is_eq: {out_req_is_eq}");                                
            }
            
            tor_stream_writer.write_all(&out_req).await.unwrap();
            tor_stream_writer.flush().await.unwrap();
        }
        Result::<(), anyhow::Error>::Ok(())
    }
    fn try_translate_http_request(&self, in_req_bytes: &[u8], out_req_bytes: &mut Vec<u8>) -> Result<ParseResult, anyhow::Error>{
        // first we try to parse it
        let 
        
    }
}

enum ParseResult{
    Successful,
    NeedMoreData,
    Error,
}