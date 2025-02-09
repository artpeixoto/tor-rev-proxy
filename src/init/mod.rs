use std::{
    collections::{HashMap, HashSet}, fs::File, io::Write, net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4}, path::{Path, PathBuf}, str::FromStr, time::Duration
};

use arti_client::{TorClient, TorClientBuilder};
use clap::Parser;
use config::{Config, ValueKind};
use log::warn;
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpSocket};
use vec1::Vec1;

use crate::{
    main_process::{connection::ConnectionConfig, listener::{ClientSocketsConfig, Listener}, manager::ClientsPermissionList, sockets::client_sockets::{SocketConfig, TlsAcceptorConfig}}, renames::TorSocket, tools::{permission_list::PermissionList, traffic_limiter::TrafficRate, used_in::UsedIn }, types::client_addr::ClientAddr
};

pub fn get_init() -> Init {
	// first, get the arguments from the user call,
    let args = InitArgs::parse();

    
    let config_path = PathBuf::from_str(&args.configs_file).unwrap();
    if config_path.exists(){
        read_init_from_file(&args.configs_file)
    } else {
        // then, we must search for the base configuration file, and if none is found, create a new one with a few defaults.
        let default_init = Init { 
            permission_list: PermissionList::Block(HashSet::new()),
            conn_cfgs: ConnectionConfig { 
                max_traffic_rate: TrafficRate::from_kbps(1024), 
                poll_rate_duration: Duration::from_secs_f32(0.1), 
                timeout: Duration::from_secs(5), 
            }, 
            client_sockets: ClientSocketsConfig {
                tcp_socket_config: Some(SocketConfig{
                    max_clients: 32000,
                    port: 80,
                }),
                tls_socket_config: None,
            }
        };
        let default_init_json = r#"{
            "permission_list": {
                "type": "blocklist",
                "items": []
            },
            "client_sockets" :{ 
                "tcp_socket": {
                    "max_simultaneous_clients": 32000,
                    "port": 80
                },
                "tls_socket": null
            },
            "connections": {
                "poll_rate_hz": 10.0,
                "timeout_ms": 5000.0,
                "max_traffic_rate_kbps": 1024
            }
        }"#;

        warn!("No configuration file found. Writing a sample file and using default values");
        
        File::create_new(config_path).unwrap().write_all(default_init_json.as_bytes()).unwrap();
        default_init
    }
}
pub fn read_init_from_file(path: &str) -> Init{
    let config = Config::builder().add_source(config::File::with_name(path)).build().unwrap();
    let client_sockets_config_map = config.get_table("client_sockets").unwrap_or_default();
    
    let client_sockets_config = ClientSocketsConfig{
        tcp_socket_config: { 
            let default_socket_config = SocketConfig{
                max_clients: 2000,
                port: 80
            };
            client_sockets_config_map
                .get("tcp_socket")
                .map(|socket_config| {
                    if socket_config.kind == ValueKind::Nil { 
                        return None
                    } else {
                        let map = socket_config.clone().into_table().unwrap();
                        Some(SocketConfig{
                            max_clients: map.get("max_simultaneous_clients").map(|val| val.clone().into_int().unwrap() as u32).unwrap_or(default_socket_config.max_clients),
                            port: map.get("port").map(|p| p.clone().into_int().unwrap() as u16).unwrap_or(default_socket_config.port) as u16,
                        })
                    }
                })
                .unwrap_or(Some(default_socket_config))
        },
        tls_socket_config: {
            client_sockets_config_map
                .get("tls_socket")
                .map(|tls_socket_config| {
                    if tls_socket_config.kind == ValueKind::Nil { 
                        return None;
                    } else {
                        let map = tls_socket_config.clone().into_table().unwrap();
                        let default_socket_config = SocketConfig{
                            max_clients: 32000,
                            port: 413,
                        };
                        let socket_config = 
                            map.get("socket_config")
                            .map(|socket_config|{
                                let map = socket_config.clone().into_table().unwrap();
                                SocketConfig{
                                    max_clients: 
                                        map.get("max_simultaneous_clients")
                                        .map(|val| val.clone().into_int().unwrap() as u32)
                                        .unwrap_or(default_socket_config.max_clients),

                                    port: map
                                        .get("port")
                                        .map(|p| p.clone().into_int().unwrap() as u16)
                                        .unwrap_or(default_socket_config.port) ,
                                }
                            })
                            .unwrap_or(default_socket_config);

                        let map = map.get("tls_config").unwrap().clone().into_table().unwrap();
                        let tls_config = TlsAcceptorConfig{
                            certificate_file: PathBuf::from_str(&map.get("certificate_file_path").unwrap().clone().into_string().unwrap()).unwrap(),
                            key_file: PathBuf::from_str(&map.get("key_file_path").unwrap().clone().into_string().unwrap()).unwrap(),
                        };

                        Some(( socket_config, tls_config ))
                    }
                })
                .unwrap_or(None)
        },
    };
    let connections_config = {
        let default_conn_config = ConnectionConfig{
            poll_rate_duration: Duration::from_secs_f32(0.1),
            timeout: Duration::from_secs(5),
            max_traffic_rate: TrafficRate::from_kbps(1024)
        };
        let config = 
            config
            .get_table("connections")
            .map(|map|{
                ConnectionConfig{
                    poll_rate_duration: map.get("poll_rate_hz").map(|val| val.clone().into_float().unwrap().used_in(|f| 1.0 / f).used_in(Duration::from_secs_f64)).unwrap_or(default_conn_config.poll_rate_duration.clone()),
                    timeout: map.get("timeout_ms").map(|val| val.clone().into_float().unwrap().used_in(|f| Duration::from_secs_f64(f / 1000.0 ))).unwrap_or(default_conn_config.timeout.clone()),
                    max_traffic_rate: map.get("max_traffic_rate_kbps").map(|val| val.clone().into_int().unwrap().used_in(|v| TrafficRate::from_kbps(v as u32))).unwrap_or(default_conn_config.max_traffic_rate.clone())
                }
            })
            .unwrap_or(default_conn_config);
        
        config
    };
    let permission_list_config = {
        config.get_table("permission_list")
            .map(|map|{
                let mut val = match map.get("type").unwrap().clone().into_string() .unwrap().as_str(){
                    "blocklist" => PermissionList::Block(HashSet::new()),
                    "allowlist" => PermissionList::Allow(HashSet::new()),
                    _ => panic!("invalid permission list type. Allowed options are 'blocklist' and 'allowlist'.")
                };
                let set = match &mut val{
                    PermissionList::Block(hash_set) =>hash_set,
                    PermissionList::Allow(hash_set) => hash_set,
                };

                set.extend( map.get("items").unwrap().clone().into_array().unwrap().iter().map(|el|
                  ClientAddr(IpAddr::from_str(&el.clone().into_string().unwrap()).unwrap())
                ));

                val
            })
            .unwrap_or(PermissionList::Block(Default::default()))
    };
    Init { permission_list: permission_list_config, client_sockets: client_sockets_config, conn_cfgs: connections_config }
}



#[derive(Serialize, Deserialize)]
pub struct Init {
    pub permission_list			: ClientsPermissionList,
    pub client_sockets          : ClientSocketsConfig,      
    pub conn_cfgs				: ConnectionConfig,
}


impl Init {
    
}

// Simple onion to tcp service router
#[derive(clap::Parser)]
pub struct InitArgs {
    #[arg(
        long="config-file", 
        short='c', 
        default_value_t={"./tor-rev-proxy.config.json".to_string()}
    )]
    configs_file: String,
}
