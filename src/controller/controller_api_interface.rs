use super::ControllerInterface;


pub struct ApiControllerInterface{

}
impl ApiControllerInterface{
	pub async fn run(&mut self) -> Result<(), anyhow::Error>{
		todo!()
	}
}
impl ControllerInterface for ApiControllerInterface {
	fn connect_to_controller(&mut self,controller: &super::Controller ) {
		std::todo!()
	}
} 