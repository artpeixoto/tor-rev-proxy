use std::{future::{Future, IntoFuture}, pin::Pin, task::{Context, Poll, Waker}};

use ::futures::FutureExt;
use tokio::task::{AbortHandle, JoinError, JoinHandle};

// AoD means Abort on Drop. This is a handle to a task, that, if dropped, will also
// terminate said task. This is useful to ensure that the said task is actually dropped

pub struct AodHandle<T>(
	JoinHandle<T>
);
impl<T> Future for AodHandle<T>{
	type Output = Result<T, JoinError>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		self.as_mut().0.poll_unpin(cx)
	}
}
impl<T> AodHandle<T>{
	pub fn close(mut self) -> TaskAbortedRes<T>{
		if self.0.is_finished(){
			let noop_waker = futures::task::noop_waker_ref();
			let mut cx = Context::from_waker(&noop_waker);
			// although the task is finished, it could be possible that some work remains to be done.
			// we finish that and get the result
			'FINISH_TASK: loop {
				if let Poll::Ready(res) = self.0.poll_unpin(&mut cx){
					break 'FINISH_TASK match res{
						Ok(successful_res) => TaskAbortedRes::FinishedSuccessfully(successful_res),
						Err(error_res) => TaskAbortedRes::FinishedWithError(error_res),
					}
				}
			}
		} else {
			self.0.abort();
			TaskAbortedRes::Aborted
		}
	}
}

#[derive(Debug)]
pub enum TaskAbortedRes<T>{
	Aborted,
	FinishedWithError(JoinError),
	FinishedSuccessfully(T)
}

impl<T> Drop for AodHandle<T>{
	fn drop(&mut self) {
		self.0.abort();
	}
}

pub trait AodHandleExt{
	type R;
	fn aod_handle(self) -> AodHandle<Self::R>; 
}

impl<T> AodHandleExt for JoinHandle<T>{
	type R = T;
	fn aod_handle(self) -> AodHandle<T> {
		AodHandle(self)
	}
}

