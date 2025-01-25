use std::{borrow::Cow, future::IntoFuture, net::{IpAddr, SocketAddr}, str::{from_utf8, FromStr}, sync::{Arc, Weak}, time::Duration};
use crate::{host::HostAddrGetter, obliterate, renames::*, tools::{safe_vec::SafeVec, traffic_direction::TrafficDirection}, types::{abort_on_drop_handle::{AbortOnDropHandle, AbortOnDropHandleExt}, client_addr::ClientAddr, endpoint::{self, Endpoint}, traffic_rate::TrafficRate}};
use arti_client::TorClient;
use either::Either::{self, Left, Right};
use futures::{future::{self, select}, FutureExt};
use httparse::Status;
use replace_with::replace_with;
use tokio::{io::{AsyncReadExt, AsyncWriteExt, Interest}, net::TcpStream, select, sync::{broadcast::Sender, OnceCell}, task::{AbortHandle, JoinHandle}};
use tor_rtcompat::tokio::PreferredRuntime;
use url::Url;
use uuid::Uuid;


pub struct ConnectionCfgs{
    pub traffic_max_rate   	: TrafficRate,
    pub poll_rate_duration  : Duration,
    pub timeout             : Duration
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ConnectionEvt{
    ConnectionTaskInitiated,
	Traffic{
		dir			: TrafficDirection, 
		other_part	: Endpoint, 
		data_len	: usize
	},	
    ConnectionClosed,
}

pub struct Connection{
    tor_to_client   		: TorToClient,
    client_to_tor   		: ClientToTor,
	evt_sender				: BroadcastSender<ConnectionEvt>,
}

impl Connection{
	pub async fn run(self) -> Result<(), anyhow::Error>{
		use future::Either::{Left, Right};

		let tor_to_client_task = Box::pin(self.tor_to_client.run_task());
		let client_to_tor_task = Box::pin(self.client_to_tor.run_task());
		let final_task = select(tor_to_client_task, client_to_tor_task);
		let res = match final_task.await{
			Left((res, _)) => {
				res
			},
			Right((res, _)) => {
				res
			},
		};

		let _ = self.evt_sender.send(ConnectionEvt::ConnectionClosed);

		res
	}
}

pub struct ConnectionInit{
	pub client_addr		: ClientAddr,
	pub client_stream	: TcpStream,
	pub host_client		: TorClient<PreferredRuntime>,	
	pub conn_configs	: WatchReceiver<ConnectionCfgs>,
	pub host_addr_getter: HostAddrGetter,

}

impl ConnectionInit{
	pub async fn build_connection(self) ->  
		Result<(ConnectionController, JoinHandle<Result<(), anyhow::Error>>), anyhow::Error>
	{
		let ( evt_sender, evt_receiver) = broadcast_channel(64);
		let ( client_reader, client_writer) = self.client_stream.into_split();
		let host_stream = self.host_client.connect(self.host_addr_getter.get_host_addr()).await?;
		let ( host_reader, host_writer,) = host_stream.split();
		let this_addr = Arc::new(OnceCell::new());

		let tor_to_client = TorToClient{
			client_writer,
			host_reader,
			evt_sender: evt_sender.clone(),
			this_addr: this_addr.clone(),
		};

		let client_to_tor = ClientToTor{
			client_reader,
			this_addr: this_addr.clone(),
			host_writer,
			host_addr_getter: self.host_addr_getter,
			evt_sender: evt_sender.clone(),
		};

		let conn = Connection{
			evt_sender: evt_sender.clone(),
			tor_to_client,
			client_to_tor,
		};
		let mut tasks = Vec::new();

		let conn_join_handle = tokio::spawn(conn.run());
		let conn_aod_handle  = conn_join_handle.get_abort_on_drop_handle();

		tasks.push(conn_aod_handle);

		let conn_ctrl = ConnectionController{
			addr			: self.client_addr,
			evt_receiver	: evt_receiver,
			tasks,
		};
		Ok((conn_ctrl, conn_join_handle))
	}
}

pub struct ConnectionController{
	addr				: ClientAddr,
	evt_receiver		: BroadcastReceiver<ConnectionEvt>,
	tasks				: Vec<AbortOnDropHandle>,
}

impl ConnectionController{
	pub fn client(&self) -> &ClientAddr{
		&self.addr
	}
	pub fn subscribe_to_events(&self) -> BroadcastReceiver<ConnectionEvt>{
		self.evt_receiver.resubscribe()
	}

	pub fn add_to_dependent_tasks<T>(&mut self, task: JoinHandle<T>) {
		self.tasks.push(task.get_abort_on_drop_handle());
	}
}

struct TorToClient{
	this_addr		: Arc<OnceCell<String>>,
	evt_sender		: BroadcastSender<ConnectionEvt>,
    client_writer   : TcpWriter,
    host_reader     : TorReader,
}


impl TorToClient { 
    async fn run_task(mut self) -> Result<(), anyhow::Error>{
		'_READ_LOOP: loop{
			let mut buf = SafeVec::new_from_vec(vec![0_u8; 32*1024]);

			let read = self.host_reader.read(buf.as_mut()).await ?;
			let read_bytes = &buf.as_ref()[0..read];
			
			self.client_writer.write_all(read_bytes).await?;
		}
    }
}

struct ClientToTor{
    this_addr       : Arc<OnceCell<String>>,
    host_addr_getter: HostAddrGetter,
	evt_sender		: BroadcastSender<ConnectionEvt>,

    client_reader   : TcpReader,
    host_writer     : TorWriter,
}


impl ClientToTor{
    pub async fn run_task(mut self) -> Result<(), anyhow::Error>{
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
					// This happens when the client reader has nothing or it is disconnected. We must test each
                }

                let in_req_bytes = &in_req_buf.as_ref()[0..read];

                // first, we try to manipulate the content as http. 
                let mut headers_buf = [httparse::EMPTY_HEADER; 128];
                let mut in_req      = httparse::Request::new(&mut headers_buf);

                match in_req.parse(in_req_bytes){
                    Ok(Status::Complete(body_start)) => {
                        // write first line
                        if let Some(m) = in_req.method.as_ref(){                            
                            self.host_writer.write_all(m.as_bytes()).await.unwrap();
                            self.host_writer.write_all(b" ").await.unwrap();
                        }

                        if let Some(p) = in_req.path.as_ref(){
                            self.host_writer.write_all(p.as_bytes()).await.unwrap();
                            self.host_writer.write_all(b" ").await.unwrap();
                        };

                        self.host_writer.write_all(b"HTTP/1.1\r\n").await.unwrap();                        

                        //write headers
                        for header in in_req.headers{
                            let header_value = match header.name{
                                "Host" => {
									'INITIALIZE_THIS_ADDR:{
										if !self.this_addr.initialized() {
											let Ok(addr_value) = from_utf8(header.value).map(|x| x.to_string()) else {break 'INITIALIZE_THIS_ADDR};

											self.this_addr.set(addr_value)?;
										}
									}
                                    Cow::Borrowed(self.host_addr_getter.get_host_addr().as_bytes())
                                },
                                "Referer" => {
                                    let mut url = Url::from_str(from_utf8(header.value)?)?;

                                    url.set_host(Some(self.host_addr_getter.get_host_addr())).unwrap();

                                    Cow::Owned(url.to_string().into_bytes())
                                }
                                _ => Cow::Borrowed(header.value)
                            };
                            self.host_writer.write_all(header.name.as_bytes()).await.unwrap();
                            self.host_writer.write_all(b": ").await.unwrap();
                            self.host_writer.write_all(&header_value).await.unwrap();
                            self.host_writer.write_all(b"\r\n").await.unwrap();
                        }
                        self.host_writer.write_all(b"\r\n").await.unwrap();

                        let body_bytes = &in_req_bytes[body_start..];
                        self.host_writer.write_all(body_bytes).await.unwrap();
                        self.host_writer.flush().await.unwrap();
                        continue 'TASK_LOOP;
                    },
                    Ok(Status::Partial)            => {
                        // incomplete, lets read more. 
                        in_req_buf.extend_with_element(0_u8, 8_usize*1024);
                        continue 'READ_AND_TRANSLATE;
                    },
                    Err(_e)                         => {
                        // parsing failed. In this case, there is no need to do anything. We just forward the message as is
                        self.host_writer.write_all(in_req_bytes).await?;
                        self.host_writer.flush().await.unwrap();
                        continue 'TASK_LOOP;
                    },
                };
            }
        }

        Result::<(), anyhow::Error>::Ok(())
    }
}
pub struct RateLimiter{

}
impl RateLimiter{
	pub fn update_rate(&mut self, new_rate: &Duration){
		todo!()
	}
	pub async fn limit_rate(&mut self, data_bytes_count: usize) {
		todo!()
	}
}