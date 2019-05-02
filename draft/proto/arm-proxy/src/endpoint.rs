use std::hash::{Hash};
use std::fmt::Debug;

pub type EndpointRep = String;
type Host = String;
type Port = u16;

pub trait Endpoint : Sized + Hash + Eq + Debug {
    fn rep(&self) -> EndpointRep;
}

pub trait HostEndpointIface : Endpoint {
    fn host(&self) -> Host;
}

pub trait PortEndpointIface : Endpoint {
    fn port(&self) -> Port; 
}

#[derive(Hash)]
#[derive(PartialEq, Eq)]
#[derive(Debug)]
pub struct HostEndpoint {
    host : Host,
}
impl HostEndpointIface for HostEndpoint {
    fn host(&self) -> Host {
        self.host.clone()
    }
}
impl Endpoint for HostEndpoint {
    fn rep(&self) -> EndpointRep {
        self.host()
    }
}
impl HostEndpoint {
    pub fn from_url_string(url: &str) -> HostEndpoint {
        let split = url.split(":").collect::<Vec<&str>>();
        HostEndpoint {
            host : split[0].to_string(),
        }
    }
}

#[derive(Hash)]
#[derive(PartialEq, Eq)]
#[derive(Debug)]
pub struct HostPortEndpoint {
    host : Host,
    port : Port
}
impl HostEndpointIface for HostPortEndpoint {
    fn host(&self) -> Host {
        self.host.clone()
    }
}
impl PortEndpointIface for HostPortEndpoint {
    fn port(&self) -> Port {
        self.port
    }
}
impl Endpoint for HostPortEndpoint {
    fn rep(&self) -> EndpointRep {
        (self.host() + ":" + &self.port().to_string())
    }
}
impl HostPortEndpoint {
    pub fn from_url_string(url: &str) -> HostPortEndpoint {
        let split = url.split(":").collect::<Vec<&str>>();
        HostPortEndpoint {
            host : split[0].to_string(),
            port : parse_port(&split[1])
        }
    }
}

pub fn parse_port(s: &str) -> u16 {
    s.parse().expect(&format!("bad port: {}", s))
}
