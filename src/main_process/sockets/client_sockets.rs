use std::fs::File;
use std::io::{self, Read};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use anyhow::bail;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{tcp, TcpListener, TcpSocket};
use tokio::net::{TcpStream};
use tokio::select;
use tokio_native_tls::native_tls::Identity;
use tokio_native_tls::TlsAcceptor;
use tor_rtcompat::tls;


fn build_tls_acceptor(config: &TlsAcceptorConfig)-> Result<TlsAcceptor, anyhow::Error>{
	use tokio_native_tls::native_tls::TlsAcceptor as InnerTlsAcceptor;
	let certificate_bytes = File::open(&config.certificate_file)?.bytes().collect::<Result<Vec<u8>, _>>()?;
	let key_bytes = File::open(&config.key_file)?.bytes().collect::<Result<Vec<u8>, _>>()?;
	let identity = Identity::from_pkcs8(&certificate_bytes,&key_bytes)?;
	let acceptor:TlsAcceptor = InnerTlsAcceptor::new(identity)?.into();

	Ok(acceptor)
}

fn build_tcp_socket(socket_config: &SocketConfig) -> Result<TcpListener, anyhow::Error> {
	let socket = TcpSocket::new_v4()?;
	socket.bind(
		SocketAddr::V4( SocketAddrV4::new(
			Ipv4Addr::UNSPECIFIED, socket_config.port
		) )
	)? ;
	Ok(socket.listen(socket_config.max_clients)?)
}

pub enum ClientSockets{
	OnlyTcp(TcpListener),
	OnlyTls(TlsSocket),
	Both{
		tcp_socket: TcpListener,
		tls_socket: TlsSocket,
	}
}

impl ClientSockets{
	pub fn build_client_sockets(tcp_config: &Option<SocketConfig>, tls_config: & Option<(SocketConfig, TlsAcceptorConfig)>) -> Result<Self, anyhow::Error>{

		let tls_socket = 
			tls_config.as_ref().map(|tls_config| -> Result<_, anyhow::Error>{
				let tls_inner_socket = build_tcp_socket(&tls_config.0)?;
				let tls_acceptor = Arc::new(build_tls_acceptor(&tls_config.1)?);
				Ok(TlsSocket{
					tcp_socket: tls_inner_socket, 
					tls_acceptor: tls_acceptor,
				})
			})
			.transpose()?;

		let tcp_socket = 
			tcp_config.as_ref().map(|tcp_config| -> Result<_, anyhow::Error>{
				let tcp_socket = build_tcp_socket(tcp_config)?;
				Ok(tcp_socket)
			})
			.transpose()?;

		let res = match (tcp_socket, tls_socket) {
			(None, None) 		=> {
				bail!("At least one of the socket configurations must be present")
			}
			(Some(tcp_socket), None) => {
				ClientSockets::OnlyTcp(tcp_socket)
			}
			(None, Some(tls_socket)) =>{
				ClientSockets::OnlyTls(tls_socket)
			},
			(Some(tcp), Some(tls)) 	=> {
				ClientSockets::Both { tcp_socket: tcp, tls_socket: tls }
			},
		};

		Ok(res)
	}

	pub async fn accept(&mut self) -> Result<(ClientStream, SocketAddr), anyhow::Error>{
		match self{
			ClientSockets::OnlyTcp(tcp_socket) => {
				let tcp_res = tcp_socket.accept().await;
				let (tcp_stream, addr) = tcp_res?;
				Ok((ClientStream::TcpStream(tcp_stream), addr))				
			},
			ClientSockets::OnlyTls(tls_socket) => {
				let (tls_stream, addr) = tls_socket.accept().await?;
				Ok((ClientStream::TlsStream(tls_stream), addr))
			},
			ClientSockets::Both { tcp_socket, tls_socket } => {select!(
				tcp_res = Box::pin(tcp_socket.accept()) => {
					let (tcp_stream, addr) = tcp_res?;
					Ok((ClientStream::TcpStream(tcp_stream), addr))				
				},
				tls_res = Box::pin(tls_socket.accept()) => {
					let (tls_stream, addr) = tls_res?;
					Ok((ClientStream::TlsStream(tls_stream), addr))
				},
			)},
		}
	}
}

pub enum ClientStream{
	TcpStream(TcpStream),
	TlsStream(TlsStream),
}
pub enum ClientStreamType{
	Tcp, Tls
}

impl ClientStream{
	pub fn get_type(&self) -> ClientStreamType{
		match self{
			ClientStream::TcpStream(_) => ClientStreamType::Tcp,
			ClientStream::TlsStream(_) => ClientStreamType::Tls,
		}
	}
}


impl AsyncWrite for ClientStream {
	fn poll_write(
		self: Pin<&mut Self>,
		cx  : &mut Context<'_>,
		buf : &[u8],
	) -> Poll<Result<usize, io::Error>> {
		match self.get_mut(){
			ClientStream::TcpStream(tcp_stream) => Pin::new(tcp_stream).poll_write(cx, buf),
			ClientStream::TlsStream(tls_stream) => Pin::new(tls_stream).poll_write(cx, buf),
		}
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
		match self.get_mut(){
			ClientStream::TcpStream(tcp_stream) => Pin::new(tcp_stream).poll_flush(cx),
			ClientStream::TlsStream(tls_stream) => Pin::new(tls_stream).poll_flush(cx),
		}
	}

	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
		match self.get_mut(){
			ClientStream::TcpStream(tcp_stream) => Pin::new(tcp_stream).poll_shutdown(cx),
			ClientStream::TlsStream(tls_stream) => Pin::new(tls_stream).poll_shutdown(cx),
		}	
	}
}

impl AsyncRead for ClientStream{
	fn poll_read(
		self: Pin<&mut Self>,
		cx	: &mut Context<'_>,
		buf	: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		match self.get_mut(){
			ClientStream::TcpStream(tcp_stream) => Pin::new(tcp_stream).poll_read(cx, buf),
			ClientStream::TlsStream(tls_stream) => Pin::new(tls_stream).poll_read(cx, buf),
		}
	}
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SocketConfig{
	pub max_clients	: u32, 
	pub port		: u16,
}

pub struct TlsSocket{
	tcp_socket		: TcpListener,
	tls_acceptor	: Arc<TlsAcceptor>,
}


#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TlsAcceptorConfig{
	pub certificate_file : PathBuf,
	pub key_file		 : PathBuf,
}
	

pub type TlsStream = tokio_native_tls::TlsStream<TcpStream>;

impl TlsSocket{
	pub async fn accept(&mut self) -> Result<(TlsStream, SocketAddr), anyhow::Error>{
		let (tcp_stream, client_socket_addr) = self.tcp_socket.accept().await?;
		let tls_stream = self.tls_acceptor.clone().accept(tcp_stream).await?;	
		Ok((tls_stream, client_socket_addr))
	}
}
