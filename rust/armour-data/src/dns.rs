//! DNS support for Armour policies
use super::Stop;
use actix::prelude::*;

#[derive(Default)]
pub struct Resolver;

// impl Resolver {
//     fn async_resolver() -> (
//         trust_dns_resolver::AsyncResolver,
//         Box<Future<Item = (), Error = ()>>,
//     ) {
//         // if let Ok((resolver, fut)) = AsyncResolver::from_system_conf() {
//         //     (resolver, Box::new(fut))
//         // } else {
//         //     let (resolver, fut) =
//         //         AsyncResolver::new(ResolverConfig::default(), ResolverOpts::default());
//         //     (resolver, Box::new(fut))
//         // }
//         let mut opts = ResolverOpts::default();
//         opts.timeout = std::time::Duration::from_secs(1);
//         let (resolver, fut) = AsyncResolver::new(ResolverConfig::new(), opts);
//         (resolver, Box::new(fut))
//     }
// }

impl Actor for Resolver {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("started Resolver")
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("stopped Resolver")
    }
}

impl Handler<Stop> for Resolver {
    type Result = ();
    fn handle(&mut self, _msg: Stop, ctx: &mut Context<Self>) -> Self::Result {
        ctx.stop()
    }
}

pub struct Lookup(pub Vec<String>);

// impl Message for Lookup {
//     type Result = Result<Vec<HashSet<std::net::IpAddr>>, ()>;
// }

// impl Handler<Lookup> for Resolver {
//     type Result = Box<dyn Future<Item = Vec<HashSet<std::net::IpAddr>>, Error = ()>>;
//     fn handle(&mut self, msg: Lookup, _ctx: &mut Context<Self>) -> Self::Result {
//         let (resolver, background_fut) = Resolver::async_resolver();
//         Box::new(background_fut.and_then(move |()| {
//             stream::futures_ordered(msg.0.into_iter().map(|ip| {
//                 resolver.ipv4_lookup(ip).then(|x| {
//                     if let Ok(ips) = x {
//                         future::ok(ips.into_iter().map(std::net::IpAddr::V4).collect())
//                     } else {
//                         future::ok(HashSet::new())
//                     }
//                 })
//             }))
//             .collect()
//         }))
//     }
// }

impl Message for Lookup {
    type Result = Result<Vec<Vec<std::net::IpAddr>>, ()>;
}

impl Handler<Lookup> for Resolver {
    type Result = Result<Vec<Vec<std::net::IpAddr>>, ()>;
    fn handle(&mut self, msg: Lookup, _ctx: &mut Context<Self>) -> Self::Result {
        Ok(msg
            .0
            .iter()
            .map(|name| dns_lookup::lookup_host(name).unwrap_or_else(|_| Vec::new()))
            .collect())
    }
}

pub struct RevLookup(pub Vec<std::net::IpAddr>);

// impl Message for RevLookup {
//     type Result = Result<Vec<HashSet<String>>, ()>;
// }

// impl Handler<RevLookup> for Resolver {
//     type Result = Box<dyn Future<Item = Vec<HashSet<String>>, Error = ()>>;
//     fn handle(&mut self, msg: RevLookup, _ctx: &mut Context<Self>) -> Self::Result {
//         let (resolver, background_fut) = Resolver::async_resolver();
//         Box::new(background_fut.and_then(move |()| {
//             stream::futures_ordered(msg.0.into_iter().map(|ip| {
//                 log::debug!("rev lookup: {}", ip);
//                 let mut names = HashSet::new();
//                 if let Ok(name) = dns_lookup::lookup_addr(&ip) {
//                     names.insert(name);
//                 }
//                 log::debug!("here");
//                 resolver
//                     .reverse_lookup(ip)
//                     .and_then(move |lookup| {
//                         future::ok(lookup.into_iter().map(|name| name.to_utf8()).collect())
//                     })
//                     .or_else(move |_err| future::ok::<_, ()>(names))
//             }))
//             .collect()
//         }))
//     }
// }

impl Message for RevLookup {
    type Result = Result<Vec<Option<String>>, ()>;
}

impl Handler<RevLookup> for Resolver {
    type Result = Result<Vec<Option<String>>, ()>;
    fn handle(&mut self, msg: RevLookup, _ctx: &mut Context<Self>) -> Self::Result {
        Ok(msg
            .0
            .iter()
            .map(|ip| dns_lookup::lookup_addr(ip).ok())
            .collect())
    }
}
