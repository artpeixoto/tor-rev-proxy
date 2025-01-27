use std::time::Duration;

use tokio::time::Instant;

pub struct RateLimiter{
	min_dur	: Duration,
	last_evt: Option<Instant>,
}

impl RateLimiter{
	pub async fn limit_rate(&mut self) {
		let mut now = Instant::now();
		if let Some(last_evt_time) = self.last_evt.take(){
			let next_allowed_time = last_evt_time + self.min_dur;
			if next_allowed_time > now  {
				tokio::time::sleep_until(next_allowed_time).await;
				now = Instant::now();
			}
		}	
		self.last_evt = Some(now);
	}
	pub fn change_rate(&mut self, new_rate_in_hertz: f32) {
		self.min_dur = Self::get_duration(new_rate_in_hertz);
	}
	fn get_duration(rate_in_hertz: f32) -> Duration{
		if rate_in_hertz < 0.0 {panic!()}
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

