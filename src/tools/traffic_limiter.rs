use std::time::Duration;
use serde::{Deserialize, Serialize};

pub struct TrafficLimiter{
	max_rate				: TrafficRate,
	accumulated_duration		: Option<Duration>
}

impl TrafficLimiter{
	pub fn add_traffic(&mut self, byte_count: usize) {
		let duration = self.max_rate.get_duration_per_kilobyte() * (byte_count as f32 / 1024.0) ;

		
	
		self.accumulated_duration = Some({
			self.accumulated_duration.take().unwrap_or(Duration::ZERO)  
		})
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
