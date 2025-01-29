use std::{ future::IntoFuture, net::{IpAddr, SocketAddr}, ops::Index, str::{from_utf8, FromStr}, sync::{Arc, Weak}, time::Duration};
use crate::{host::HostAddrGetter, obliterate, renames::*, tools::{abort_on_drop_handle::{AodHandle, AodHandleExt}, safe_vec::{SafeString, SafeVec}, traffic_direction::TrafficDirection, traffic_limiter::TrafficRate}, types::{ client_addr::ClientAddr, endpoint::{self, Endpoint}}};
use either::Either::{self, Left, Right};
use replace_with::replace_with;
use serde::{Deserialize, Serialize};
use tokio::{io::{AsyncReadExt, AsyncWriteExt, Interest}, net::TcpStream, select, sync::{broadcast::Sender, OnceCell}, task::{AbortHandle, JoinHandle}};
use url::Url;
use webparse::{HeaderName, WebError};

use super::sockets::client_sockets::ClientStream;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize , Debug)]
pub struct ConnectionConfig{
	pub traffic_max_rate   	: TrafficRate,
    pub poll_rate_duration  : Duration,
    pub timeout             : Duration
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
pub enum ConnectionEvent{
    ConnectionEstabilished,
	TrafficHappened{
		dir			: TrafficDirection, 
		other_part	: Endpoint, 
		data_len	: usize
	},	
    ConnectionClosed(),
}

pub enum ConnectionClosedReason{
	TimedOut,
}

pub struct ConnectionTaskInit{
	event_sender	: BroadcastSender<ConnectionEvent>,
	config_reader   : WatchReceiver<ConnectionConfig>,

	client_stream	: ClientStream,
	host_stream		: TorStream,

	host_addr_getter: HostAddrGetter,
}

pub struct ConnectionTaskHandle(
	AodHandle<Result<(), anyhow::Error>>
);

impl ConnectionTaskInit{
	pub fn run(self) -> ConnectionTaskHandle{
		ConnectionTaskHandle(tokio::spawn(self.run_internal()).aod_handle())
	}

	async fn run_internal(mut self) -> Result<(), anyhow::Error>{
		#![allow(unreachable_code)]

		let mut client_read_buf = SafeVec::new_from_vec(vec![0_u8;16*1024]);
		let mut host_read_buf   = SafeVec::new_from_vec(vec![0_u8;64*1024]);

		'_TASK_LOOP: loop {
			select!{
				client_read_res = self.client_stream.read( client_read_buf.as_mut()) => {
					let read_count = client_read_res?;	
					self.send_from_client_to_host(&mut client_read_buf, read_count).await?;
				},
				host_read_res = self.host_stream.read(host_read_buf.as_mut()) => {
					let read_count = host_read_res?;
					self.send_from_host_to_client(&mut host_read_buf, read_count).await?;
				}
			}
		}

		let _ = self.event_sender.send(ConnectionEvent::ConnectionClosed());

		Ok(())
	}

	async fn send_from_client_to_host(
		&mut self, 
		buf			: &mut SafeVec<u8>, 
		mut read_count	: usize
	) -> Result<(), anyhow::Error>{

		let parsed_req = 'READ_LOOP: loop {
			let mut req = webparse::Request::new();
			let parse_res = req.parse(&buf[0..read_count]);
			
			// first, we try to parse the content as http. 
			match parse_res{
				Ok(body_start) => {
					break 'READ_LOOP Left((req, body_start));
				},
				Err(e) if e.is_partial() => {
					buf.extend_with_elements(0_u8, 8*1024);
					read_count += self.client_stream.read(&mut buf[read_count..]).await?;
					continue 'READ_LOOP;
				},
				Err(e) => {
					break 'READ_LOOP Right(&buf.as_ref()[0..read_count]);
				}
			};
		};
		match parsed_req {
			Right(bytes) => {
				// parsing failed. In this case, there is no need to do anything. We just forward the message as is
				self.host_stream.write_all(bytes).await?;
				Ok(())
			}
			Left((headers, body_start)) => {
				// The message is could be parsed as http, and therefore we must process it.

				// write first line
				self.host_stream.write_all(headers.method().as_str().as_bytes()).await.unwrap();

				self.host_stream.write_all(b" ").await.unwrap();

				self.host_stream.write_all(headers.path().as_bytes()).await.unwrap();
				self.host_stream.write_all(b" ").await.unwrap();
				self.host_stream.write_all(headers.version().as_str().as_bytes()).await.unwrap();

				self.host_stream.write_all(b"\r\n").await.unwrap();                        

				//write headers
				for (header_name, header_value) in headers.headers().iter(){
					async fn write_header<'a>(
						host_writer : &mut TorStream,
						header_name : &'a str, 
						header_value: &'a [u8]
					)-> Result<(), anyhow::Error> { 
						host_writer.write_all(header_name.as_bytes()).await?;
						host_writer.write_all(b": ").await?;
						host_writer.write_all(&header_value).await?;
						host_writer.write_all(b"\r\n").await?;
						Ok(())
					}

					match header_name.name(){
						"Host" => { 
							let host_value = self.host_addr_getter.get_host_addr();
							write_header(
									&mut self.host_stream, 
									HeaderName::HOST.name(),  
									host_value.as_ref().as_bytes()
								)
								.await?;
						},
						"Referer" => {
							let mut url = Url::from_str(from_utf8(header_value.as_bytes())?)?;

							url.set_host(Some(self.host_addr_getter.get_host_addr().as_ref())).unwrap();

							let safe_url_string =SafeString::from_string( url.into());

							write_header(&mut self.host_stream, header_name.name(), safe_url_string.as_ref().as_bytes()).await?;
						}
						header_name => {
							write_header(&mut self.host_stream, header_name, header_value.as_bytes())
							.await?;
						}
					};
				}
				self.host_stream.write_all(b"\r\n").await.unwrap();

				//write body
				let body = &buf[(body_start) .. (body_start + headers.get_body_len() as usize)];
				self.host_stream.write_all(body).await.unwrap();

				//flush
				self.host_stream.flush().await.unwrap();

				Ok(())
			},
		}
	}

	async fn send_from_host_to_client(
		&mut self, 
		buf				: &mut SafeVec<u8>,
		read_count		: usize
	) -> Result<(), anyhow::Error>{
		let read_bytes = &buf.as_ref()[0..read_count];
		self.client_stream.write_all(read_bytes).await?;
		Ok(())
	}
}

pub struct ConnectionInit{
	pub client_addr		: ClientAddr,

	pub client_stream	: ClientStream,
	pub host_socket		: TorSocket,	

	pub config_reader	: WatchReceiver<ConnectionConfig>,
	pub host_addr_getter: HostAddrGetter,
	pub input_port		: u16,
}

impl ConnectionInit{
	pub async fn build_connection(self) ->  
		Result<(ConnectionController, ConnectionTaskHandle), anyhow::Error>
	{
		let (evt_sender, evt_receiver) = broadcast_channel(64);

		let host_stream = self.host_socket.connect((self.host_addr_getter.get_host_addr().as_ref(), self.input_port)).await?;

		let conn = ConnectionTaskInit{
			event_sender: evt_sender,
			config_reader: self.config_reader,
			client_stream: self.client_stream,
			host_stream,
			host_addr_getter: self.host_addr_getter,
		};

		let tasks = Vec::new();
		let conn_handle = conn.run();

		let conn_ctrl = ConnectionController{
			addr			: self.client_addr,
			evt_receiver	: evt_receiver,
			tasks,
		};
		
		Ok((conn_ctrl, conn_handle))
	}
}

pub struct ConnectionController{
	addr				: ClientAddr,
	evt_receiver		: BroadcastReceiver<ConnectionEvent>,
	tasks				: Vec<AodHandle<Result<(), anyhow::Error>>>,
}

impl ConnectionController{
	pub fn client_addr(&self) -> &ClientAddr{
		&self.addr
	}
	pub fn subscribe_to_events(&self) -> BroadcastReceiver<ConnectionEvent>{
		self.evt_receiver.resubscribe()
	}

	pub fn add_to_dependent_tasks<T>(&mut self, task: JoinHandle<T>) {
		self.tasks.push(task.get_abort_on_drop_handle());
	}
}


