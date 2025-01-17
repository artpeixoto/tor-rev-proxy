use crate::init::Init;

mod controller_api_interface;

pub struct Controller{

}

impl Controller{
	pub fn new(init: &Init) -> Self{
		todo!()
	}
	pub fn connect_to_main_process(&mut self, )
}

pub trait ControllerInterface{
	fn connect_to_controller(&mut self,controller: &Controller ) {
		todo!()
	}
	
}