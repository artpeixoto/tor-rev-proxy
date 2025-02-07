use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

pub struct TrafficLimiter{
	max_rate	: TrafficRate,
	deadline	: Option<Instant>
}

impl TrafficLimiter{
	pub fn add_traffic(&mut self, byte_count: usize) {
		let duration = self.max_rate.get_duration_per_kilobyte().mul_f32( (byte_count as f32 / 1024.0) );
		self.deadline = Some(Instant::now() + duration);
	}
	pub async fn limit_rate(&mut self) {
		let now = Instant::now();
		if let Some(deadline) = self.deadline.take() {
			if deadline > now {
				tokio::time::sleep_until(deadline).await;
			}
		}
	}
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, PartialOrd, Ord, Deserialize)]
pub struct TrafficRate{
	pub kilobytes_per_second: u32,
}

impl TrafficRate{
	pub fn new(kilobytes_per_second: u32) -> Self{
		Self{kilobytes_per_second}
	}
	pub fn get_duration_per_kilobyte(&self) -> Duration{
		Duration::from_secs(1) / self.kilobytes_per_second
	}
}
