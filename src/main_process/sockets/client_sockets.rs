use std::fs::File;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{tcp, TcpListener, TcpSocket};
use tokio::net::{TcpStream};
use tokio::select;
use tokio_native_tls::native_tls::Identity;
use tokio_native_tls::TlsAcceptor;
use tor_rtcompat::tls;

fn build_tls_acceptor(config: &TlsConfig)-> Result<TlsAcceptor, anyhow::Error>{
	use tokio_native_tls::native_tls::TlsAcceptor as InnerTlsAcceptor;
	let certificate_bytes = File::open(&config.certificate_file)?.bytes().collect::<Vec<u8>>();
	let key_bytes = File::open(&config.key_file)?.bytes().collect::<Vec<u8>>();
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
	Ok(socket.listen(socket_config.max_clients))
}

pub struct ClientSockets{
    tcp_socket : TcpListener,
    tls_socket : TlsSocket
}

impl ClientSockets{
	pub async fn accept(&mut self) -> Result<(ClientStream, SocketAddr), anyhow::Error>{
		select!(
			tcp_res = Box::pin(self.tcp_socket.accept()) => {
				let (tcp_stream, addr) = tcp_res?;
				Ok((ClientStream::TcpStream(tcp_stream), addr))				
			},
			tls_res = Box::pin(self.tls_socket.accept()) => {
				let (tls_stream, addr) = tls_res?;
				Ok((ClientStream::TlsStream(tls_stream), addr))
			},
		)
	}
}

pub enum ClientStream{
	TcpStream(TcpStream),
	TlsStream(TlsStream),
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SocketConfig{
	pub max_clients	: u32, 
	pub port		: u16,
}

pub struct TlsSocket{
	tcp_socket		: TcpListener,
	tls_acceptor	: Arc<TlsAcceptor>,
}


#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TlsConfig{
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
