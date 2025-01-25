use tokio::task::{AbortHandle, JoinHandle};

pub struct AbortOnDropHandle(AbortHandle);

pub trait AbortOnDropHandleExt{
	fn get_abort_on_drop_handle(&self) -> AbortOnDropHandle; 
}
impl<T> AbortOnDropHandleExt for JoinHandle<T>{
	fn get_abort_on_drop_handle(&self) -> AbortOnDropHandle {
		AbortOnDropHandle(self.abort_handle())
	}
}
impl Drop for AbortOnDropHandle{
	fn drop(&mut self) {
		self.0.abort();
	}
}