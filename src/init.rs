
pub struct Init{

}
pub fn get_init() -> Init {
	todo!();
}


// Simple onion to tcp service router
#[derive(clap::Parser)]
pub struct InitArgs{
    // Port that will receive the requests
    #[arg(short='p', long="serve-port", default_value_t=80) ]    
    pub serve_port  : u16,

    // The tor host to which the requests will be forwarded to 
    #[arg(short='H', long="host" , default_value_t=("haystak5njsmn2hqkewecpaxetahtwhsbsa64jom2k22z5afxhnpxfid.onion".to_string()))]
    pub host        : String,

    #[arg(long="host-port", short='P', default_value_t=80)]
    pub host_port   : u16
}

