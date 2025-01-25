use std::net::IpAddr;


#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct ClientAddr(pub IpAddr);

