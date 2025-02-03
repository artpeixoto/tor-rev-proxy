use crate::tools::safe_vec::SafeString;

#[derive(Clone)]
pub struct HostAddrGetter{
}

impl HostAddrGetter{
	pub fn new() -> Self{
		Self{}
	}
	pub fn get_host_addr(&self) -> SafeString{
		let enc_host: String = 
			cryptify::encrypt_string!("haystak5njsmn2hqkewecpaxetahtwhsbsa64jom2k22z5afxhnpxfid.onion:80");
		SafeString::from_string(enc_host)
	}
}	
