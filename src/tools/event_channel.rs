use std::time::SystemTime;

use chrono::{DateTime, Local, NaiveDateTime};
use tokio::sync::broadcast::{self , *};

pub use tokio::sync::broadcast::error::{RecvError, SendError, TryRecvError};
pub fn channel<T: Clone>(capacity: usize) -> (EventSender<T>, EventReceiver<T>){
	let (inner_sender, inner_receiver) = broadcast::channel(capacity);
	(EventSender{inner_sender}, EventReceiver{inner_receiver})
} 

#[derive(Clone)]
pub struct EventSender<T:Clone>{
	inner_sender: Sender<(DateTime<Local>, T)>
}

impl<T:Clone> EventSender<T>{
	pub fn send(&mut self, value: T) -> Result<usize, SendError<T>>{
		let now = SystemTime::now().into();
		self.inner_sender.send((now, value)).map_err(|SendError((_now, val))| SendError(val))
	}

	pub fn subscribe(&mut self) -> EventReceiver<T>{
		EventReceiver{inner_receiver: self.inner_sender.subscribe()}
	}
}

pub struct EventReceiver<T: Clone>{
	inner_receiver: Receiver<(DateTime<Local>, T)>
}

impl<T:Clone> EventReceiver<T>{
	pub fn try_recv(&mut self) -> Result<(DateTime<Local>, T), TryRecvError>{
		self.inner_receiver.try_recv()
	}
	pub async fn recv(&mut self) -> Result<(DateTime<Local>, T), RecvError>{
		self.inner_receiver.recv().await
	}
	pub fn resubscribe(&self) -> Self{
		Self { inner_receiver: self.inner_receiver.resubscribe() }
	}
}
