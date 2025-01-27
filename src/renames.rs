// channels
pub use tokio::sync::broadcast::{channel as broadcast_channel, Sender as BroadcastSender, Receiver as BroadcastReceiver};
pub use tokio::sync::watch::{
	channel 	as watch_channel, 
	Receiver 	as WatchReceiver, 
	Sender 		as WatchSender
};
pub use tokio::sync::mpsc::{channel as mpsc_channel, Sender as MpscSender, Receiver as MpscReceiver};
pub use tokio::sync::oneshot::{channel as oneshot_channel, Sender as OneshotSender, Receiver as OneshotReceiver};

// sockets and streams 
pub use arti_client::DataReader as TorReader;
pub use arti_client::DataStream as TorStream;
pub use arti_client::DataWriter as TorWriter;

pub use tokio::net::tcp::OwnedWriteHalf as TcpWriter;
pub use tokio::net::tcp::OwnedReadHalf as TcpReader;
use tor_rtcompat::PreferredRuntime;

pub type TorSocket = arti_client::TorClient<PreferredRuntime>;
