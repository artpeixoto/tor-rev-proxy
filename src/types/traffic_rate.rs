use std::time::Duration;

use serde::Serialize;


#[derive(Debug, Clone, PartialEq, Eq, Serialize, PartialOrd, Ord)]
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
