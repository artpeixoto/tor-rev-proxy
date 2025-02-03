use std::{future::{Future, IntoFuture}, io, ops::DerefMut, path::PathBuf, pin::Pin, sync::{Arc,  Weak}, task::{Context, Poll}};
use axum::serve::Listener;
use futures::{FutureExt, TryFutureExt};
use http::request;
use tokio::{io::AsyncRead, net::windows::named_pipe::{NamedPipeClient, *}};
use tokio::{io::{AsyncReadExt, AsyncWrite, AsyncWriteExt}, sync::{OwnedRwLockWriteGuard, RwLock} };

pub struct LocalServerSocket{
	pipe: Arc<RwLock<NamedPipeServer>>,
	addr: String,
}
pub struct LocalServerStream{
	pipe : OwnedRwLockWriteGuard<NamedPipeServer>,
}

impl LocalServerSocket{
	pub fn new(addr: String) -> Result<Self, anyhow::Error>{
		let pipe = ServerOptions::new()
			.first_pipe_instance(true)
			.pipe_mode(PipeMode::Byte)
			.create(addr.clone())?;

		Ok(LocalServerSocket { pipe: Arc::new(RwLock::new(pipe)), addr: addr.clone() })
	}
}

impl AsyncWrite for LocalServerStream{
	fn poll_write(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<Result<usize, io::Error>> {
		let mut pin_pipe = Pin::new(self.pipe.deref_mut());
		pin_pipe.as_mut().poll_write(cx, buf)
	}

	fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
		let pin_pipe = Pin::new(self.pipe.deref_mut());
		pin_pipe.poll_flush(cx)
	}

	fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
		let pin_pipe = Pin::new(self.pipe.deref_mut());
		pin_pipe.poll_shutdown(cx)
	}
}
impl AsyncRead for LocalServerStream{
	fn poll_read(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut tokio::io::ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		let pin_pipe = Pin::new(self.pipe.deref_mut());
		pin_pipe.poll_read(cx, buf)
	}
}


impl Listener for LocalServerSocket {
	type Io = LocalServerStream;

	type Addr = String;

	fn accept(&mut self) -> impl Future<Output = (Self::Io, Self::Addr)> + Send {
		let pipe = self.pipe.clone();
		async {
			let pipe_write =  pipe.write_owned().await;
			pipe_write.connect().await.unwrap();
			(LocalServerStream{pipe: pipe_write}, self.addr.clone())
		}
	}

	fn local_addr(&self) -> tokio::io::Result<Self::Addr> {
		Ok(self.addr.clone())
	}
}

pub struct LocalClientSocket (
	NamedPipeClient
);
