use std::net::IpAddr;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct ClientAddr(pub IpAddr);