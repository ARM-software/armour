use actix_web::{
    client, AsyncResponder, Error, FromRequest, HttpMessage, HttpRequest, HttpResponse, Path,
};
use futures::{future::ok as fut_ok, Future};

use super::endpoint;
use endpoint::*;

use super::policy;
use policy::*;

use url::Url;

#[derive(Debug)]
enum ForwardUrlError {
    ParseError(url::ParseError),
    ParseIntError(std::num::ParseIntError),
    // Blocked(u16),
    // ForwardPort,
    ForwardUrlError,
}

impl From<url::ParseError> for ForwardUrlError {
    fn from(err: url::ParseError) -> ForwardUrlError {
        ForwardUrlError::ParseError(err)
    }
}

impl From<std::num::ParseIntError> for ForwardUrlError {
    fn from(err: std::num::ParseIntError) -> ForwardUrlError {
        ForwardUrlError::ParseIntError(err)
    }
}

impl From<()> for ForwardUrlError {
    fn from(_err: ()) -> ForwardUrlError {
        ForwardUrlError::ForwardUrlError
    }
}

fn forward_url(req: &HttpRequest<policy::PolicyStateL3>) -> Result<Url, ForwardUrlError> {
    let full_target_name = req.headers()["HOST"].to_str().unwrap();
    let target_name = HostPortEndpoint::from_url_string(full_target_name);
    let source_name = HostEndpoint::from_url_string(&req.peer_addr().unwrap().ip().to_string());
    if req.state().validate(source_name, target_name).unwrap() {
        let info = req.connection_info();
        let url = Url::parse(&format!("{}://{}{}", info.scheme(), info.host(), req.uri())).unwrap();
        Ok(url)
    } else {
        Err(ForwardUrlError::ForwardUrlError)
    }
}

/// Forward request from client sender to a destination server
pub fn forward(
    req: &HttpRequest<policy::PolicyStateL3>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    match forward_url(req) {
        Ok(server_url) => {
            let mut forwarded_req = client::ClientRequest::build_from(req)
                .no_default_headers()
                .uri(server_url)
                .streaming(req.payload())
                .unwrap();

            if let Some(addr) = req.peer_addr() {
                match forwarded_req.headers_mut().entry("x-forwarded-for") {
                    Ok(http::header::Entry::Vacant(entry)) => {
                        let addr = format!("{}", addr.ip());
                        entry.insert(addr.parse().unwrap());
                    }
                    Ok(http::header::Entry::Occupied(mut entry)) => {
                        let addr = format!("{}, {}", entry.get().to_str().unwrap(), addr.ip());
                        entry.insert(addr.parse().unwrap());
                    }
                    _ => unreachable!(),
                }
            }

            forwarded_req
                .send()
                .map_err(Error::from)
                .and_then(construct_response)
                .responder()
        }
        // Err(ForwardUrlError::Blocked(port)) => Box::new(fut_ok(
        //     HttpResponse::Forbidden().body(&format!("access to server {} is blocked", port)),
        // )),
        Err(err) => Box::new(fut_ok(
            HttpResponse::BadRequest().body(&format!("failed to construct server URL {:?}", err)),
        )),
    }
}

/// Forward response from detination server back to client sender
fn construct_response(
    resp: client::ClientResponse,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let mut client_resp = HttpResponse::build(resp.status());
    for (header_name, header_value) in resp.headers().iter().filter(|(h, _)| *h != "connection") {
        client_resp.header(header_name.clone(), header_value.clone());
    }
    if resp.chunked().unwrap_or(false) {
        Box::new(fut_ok(client_resp.streaming(resp.payload())))
    } else {
        Box::new(
            resp.body()
                .from_err()
                .and_then(move |body| Ok(client_resp.body(body))),
        )
    }
}

/// Forward response from detination server back to client sender
pub fn allow_host(req: &HttpRequest<policy::PolicyStateL3>) -> Result<String, Error> {
    let t = Path::<(String, String, u16)>::extract(&req).unwrap();
    let source = HostEndpoint::from_url_string(&t.0.to_string());
    let fullname = t.1.to_string() + ":" + &t.2.to_string();
    let target = HostPortEndpoint::from_url_string(&fullname);
    req.state()
        .enable(source, target)
        .map(|_| "".to_string())
        .map_err(|_| actix_web::error::ErrorInternalServerError("allow host error"))
}
