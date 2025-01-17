use std::borrow::Cow;
use std::collections::LinkedList;
use std::str::from_utf8;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use httparse::Status;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf as TcpWriter;
use tokio::net::tcp::OwnedReadHalf as TcpReader;
use arti_client::DataReader as TorReader;
use arti_client::DataWriter as TorWriter;
use tokio::sync::OnceCell;
use tokio::sync::RwLock;
use url::Url;

use crate::host::HostAddrGetter;
use crate::safe_vec::SafeVec;

pub struct ConnectionCfgs{
    max_rate_duration   : Duration,
    poll_rate_duration  : Duration,
    timeout             : Duration
}



pub enum ConnectionEvt{
    Created,
    Died
}
pub enum ConnectionCmd{

}

pub struct Connection{

    this_addr    : Arc<OnceCell<String>>,
    tor_addr     : HostAddrGetter,

	parts       : (TorToClient, ClientToTor),
}



pub struct ConnectionManager{
    conns: LinkedList<Connection>
}



pub enum ConnectionListenerEvt{
    ReceivedConnReq,
    ConnectionEstabilished,
    ConnectionDenied,
}

pub struct ConnectionListener{
}



struct TorToClient{
    client_writer   : TcpWriter,
    tor_reader      : TorReader,
}


impl TorToClient { 
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

struct ClientToTor{
    this_addr       : Arc<OnceCell<String>>,

    tor_addr        : Arc<String>,
    // tor_port        : u16,  

    client_reader   : TcpReader,
    tor_writer      : TorWriter,
}


impl ClientToTor{
    pub async fn run_task(&mut self) -> Result<(), anyhow::Error>{

        'TASK_LOOP: loop{
            let mut in_req_buf      = SafeVec::new_from_vec(vec![0_u8; 8*1024]);
            let mut read            = 0;  //keeps track of the amount read

            'READ_AND_TRANSLATE: loop{
                // wait for message
                let Ok(just_read) = self.client_reader.read(&mut in_req_buf.as_mut()[read..]).await else {
                    //something happened to the connection. We just stop doing anything then
                    break 'TASK_LOOP;
                };
                
                // advances amount read
                read = read + just_read; 

                if just_read == 0 && read == 0{
                    break 'TASK_LOOP;
                }

                println!("Received message from client {read}");
                let in_req_bytes = &in_req_buf.as_ref()[0..read];

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
                                    if self.this_addr.
                                        let header_value = from_utf8(header.value).unwrap().to_string();
                                        self.this_addr.set(header_value).unwrap();
                                    }

                                    Cow::Borrowed(self.tor_addr_getter.)
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
                        in_req_buf.extend_with_element(0_u8, 8_usize*1024);
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