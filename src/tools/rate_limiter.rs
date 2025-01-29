use std::time::Duration;

use tokio::time::Instant;

use crate::obliterate;

pub struct RateLimiter{
	min_dur	: Duration,
	last_evt: Option<Instant>,
}

impl RateLimiter{
	pub fn mark_evt(&mut self){
		let mut now = Instant::now();
		self.last_evt = Some(now);
	}
	pub async fn limit_rate(&mut self) {
		let now = Instant::now();
		if let Some(last_evt_time) = &self.last_evt{
			let next_allowed_time = *last_evt_time + self.min_dur;
			if next_allowed_time > now  {
				tokio::time::sleep_until(next_allowed_time).await;
			}
		}	
	}

	pub async fn limit_rate_and_mark(&mut self) {
		self.limit_rate().await;
		self.mark_evt();
	}

	pub fn change_rate(&mut self, new_rate_in_hertz: f32) {
		self.min_dur = Self::get_duration(new_rate_in_hertz);
	}
	fn get_duration(rate_in_hertz: f32) -> Duration{
		if rate_in_hertz < 0.0 {obliterate!()}
		let min_dur = Duration::from_secs_f32(1.0/rate_in_hertz); 
		min_dur
	}
	pub fn new(rate_in_hertz: f32) -> Self{
		Self {
			min_dur	: Self::get_duration(rate_in_hertz),
			last_evt: None
		}
	}
}

