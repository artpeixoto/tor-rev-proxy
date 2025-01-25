use crate::{
    init::Init,
    main_process::{
        connection::{ConnectionCfgs, ConnectionEvt},
        connection_manager::{ConnectionManagerCfgs, ConnectionManagerEvt},
    },
    renames::{BroadcastReceiver, WatchSender},
};

pub mod controller_api_interface;

pub struct Controller {
    conn_cfg: WatchSender<ConnectionCfgs>,
    conn_mngr_cfg: WatchSender<ConnectionManagerCfgs>,
    conn_evts_rec: BroadcastReceiver<ConnectionEvt>,
    conn_mngr_evts_rec: BroadcastReceiver<ConnectionManagerEvt>,
}

impl Controller {
    pub fn new(
        conn_cfg: WatchSender<ConnectionCfgs>,
        conn_mngr_cfg: WatchSender<ConnectionManagerCfgs>,
        conn_evts_rec: BroadcastReceiver<ConnectionEvt>,
        conn_mngr_evts_rec: BroadcastReceiver<ConnectionManagerEvt>,
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
