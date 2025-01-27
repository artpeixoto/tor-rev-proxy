use crate::{
    comm_channels::{self, ControllerEndpoint}, init::Init, main_process::{
        connection::{ConnectionConfig, ConnectionEvent},
        process::{ProcessConfig, ProcessEvent},
    }, renames::{BroadcastReceiver, WatchSender}
};
pub mod tty_interface;
pub mod controller_api;

pub struct Controller {
    comm_channels: comm_channels::ControllerEndpoint,
}

impl Controller {
    pub fn new(
        conn_cfg: WatchSender<ConnectionConfig>,
        conn_mngr_cfg: WatchSender<ProcessConfig>,
        conn_evts_rec: BroadcastReceiver<ConnectionEvent>,
        conn_mngr_evts_rec: BroadcastReceiver<ProcessEvent>,
    ) -> Self {
        Self {
            conn_cfg,
            conn_mngr_cfg,
            conn_evts_rec,
            conn_mngr_evts_rec,
        }
    }
}

pub trait ControllerInterface {}
