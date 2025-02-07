use std::time::SystemTime;

use crate::renames::*;

use super::event_channel::EventReceiver;

pub trait ReceiverExt{
    type T;
    fn recv_all(&mut self) -> Result<Vec<Self::T>, anyhow::Error>;
}

impl<T: Clone> ReceiverExt for BroadcastReceiver<T>{
    type T = T; 
    fn recv_all(&mut self) -> Result<Vec<Self::T>, anyhow::Error>{
        use tokio::sync::broadcast::error::TryRecvError;
        let mut vec = Vec::new();

        'GET_EVENTS_LOOP: loop {
            match self.try_recv(){
                Ok(val) =>{
                    vec.push(val);
                    continue 'GET_EVENTS_LOOP;
                },
                Err(TryRecvError::Lagged(_)) => {continue 'GET_EVENTS_LOOP;}
                Err(TryRecvError::Closed)    => {break 'GET_EVENTS_LOOP Err(anyhow::anyhow!("Channel closed"));},
                Err(TryRecvError::Empty)     => {break 'GET_EVENTS_LOOP Ok(vec);}
            }
        }
    }
}

impl<T: Clone> ReceiverExt for EventReceiver<T>{
    type T = (SystemTime, T);

    fn recv_all(&mut self) -> Result<Vec<Self::T>, anyhow::Error> {
        use crate::tools::event_channel::TryRecvError;
        let mut vec = Vec::new();

        'GET_EVENTS_LOOP: loop {
            match self.try_recv(){
                Ok(val) =>{
                    vec.push(val);
                    continue 'GET_EVENTS_LOOP;
                },
                Err(TryRecvError::Lagged(_)) => {continue 'GET_EVENTS_LOOP;}
                Err(TryRecvError::Closed)    => {break 'GET_EVENTS_LOOP Err(anyhow::anyhow!("Channel closed"));},
                Err(TryRecvError::Empty)     => {break 'GET_EVENTS_LOOP Ok(vec);}
            }
        }
    }
}