use crate::{
    comm_channels::{self, ConfigWriters, ControllerEndpoint, EventListeners}, init::Init, main_process::{
        connection::{ConnectionConfig, ConnectionEvent},
        process::{ProcessConfig, ProcessEvent},
    }, renames::{BroadcastReceiver, WatchSender}
};

pub struct Controller {
    config_writers  : ConfigWriters,
    events_listeners: EventListeners
}
pub struct ControllerConfig{
	endpoint
}

impl Controller {
    pub fn new(
        controller_comm_endpoint: ControllerEndpoint
    ) -> Self {
        Self {
            config_writers: controller_comm_endpoint.cfg,
            events_listeners: controller_comm_endpoint.evt,
        }
    }

    pub fn change_process_config(&mut self, cfg_changer: impl FnOnce(&mut ProcessConfig)){
		self.config_writers.process.send_modify(cfg_changer);
    }
	
	pub fn change_client
}



