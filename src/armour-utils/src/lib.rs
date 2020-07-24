//! Library of Armour helper functions.

use lazy_static::lazy_static;
use std::collections::HashSet;
use std::io;
use std::net::IpAddr;

lazy_static! {
    /// Set of IP addresses associated with local interface.
    pub static ref INTERFACE_IPS: HashSet<IpAddr> = {
        let set: HashSet<String> = ["lo", "lo0", "en0", "eth0"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        if let Ok(interfaces) = get_if_addrs::get_if_addrs() {
            interfaces
                .into_iter()
                .filter_map(|i| {
                    if set.contains(&i.name) {
                        Some(i.ip())
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            HashSet::new()
        }
    };
}

/// Check if the supplied IP address correspondence with an IP of the local interface.
///
/// Used to check if a proxy is attempting to proxy itself.
pub fn own_ip(s: &IpAddr) -> bool {
    INTERFACE_IPS.contains(s)
        || match s {
            IpAddr::V4(v4) => v4.is_unspecified() || v4.is_broadcast(),
            IpAddr::V6(v6) => v6.is_unspecified(),
        }
}

/// Serialize data using [bincode](https://github.com/servo/bincode) encoder,
/// followed by [flate2](https://github.com/alexcrichton/flate2-rs) compression and
/// [base64](https://github.com/marshallpierce/rust-base64) encoding.
///
/// Can be used to transmit serializable rust data over HTTP.
pub fn bincode_gz_base64_enc<W: std::io::Write, S: serde::Serialize>(
    mut w: W,
    data: S,
) -> std::io::Result<()> {
    let mut gz_base64_enc = flate2::write::GzEncoder::new(
        base64::write::EncoderWriter::new(&mut w, base64::STANDARD),
        flate2::Compression::fast(),
    );
    bincode::serialize_into(&mut gz_base64_enc, &data)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    gz_base64_enc.finish().map(|_| ())
}

/// Deserialize data that has been encoded with [bincode_gz_base64_enc](fn.bincode_gz_base64_enc.html).
pub fn bincode_gz_base64_dec<R: std::io::Read, D: serde::de::DeserializeOwned>(
    mut r: R,
) -> Result<D, std::io::Error> {
    let mut buf = Vec::new();
    r.read_to_end(&mut buf)?;
    let bytes = base64::decode(&buf).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    bincode::deserialize_from(flate2::read::GzDecoder::new(&bytes[..]))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

pub fn parse_https_url(s: &str, default_port: u16) -> Result<url::Url, String> {
    let err = || format!("failed to parse HTTPS URL: {}", s);
    if let Ok(socket) = s.parse::<std::net::SocketAddrV4>() {
        format!("https://{}", socket)
            .parse::<url::Url>()
            .map_err(|_| err())
    } else {
        let mut url = s.parse::<url::Url>().map_err(|_| err())?;
        if url.host_str().is_some() {
            if url.scheme() == "https" {
                if url.port().is_none() {
                    url.set_port(Some(default_port)).map_err(|_| err())?
                }
                Ok(url)
            } else {
                Err(err())
            }
        } else if url.scheme() == "localhost" {
            format!("https://{}", s)
                .parse::<url::Url>()
                .map_err(|_| err())
        } else {
            Err(err())
        }
    }
}

pub fn ssl_builder<P: AsRef<std::path::Path>>(
    ca: P,
    certificate_password: &str,
    certificate: P,
    mtls: bool,
) -> Result<openssl::ssl::SslAcceptorBuilder, Box<dyn std::error::Error + Send + Sync>> {
    use openssl::ssl::{SslAcceptor, SslMethod, SslVerifyMode};
    use std::io::Read;
    let mut certificate_file = std::fs::File::open(certificate.as_ref()).map_err(|err| {
        log::warn!(
            "failed to read certificate: {}",
            certificate.as_ref().display()
        );
        err
    })?;
    let mut bytes = Vec::new();
    certificate_file.read_to_end(&mut bytes)?;
    let p12 = openssl::pkcs12::Pkcs12::from_der(&bytes)?.parse(certificate_password)?;
    let mut ssl_builder = SslAcceptor::mozilla_modern(SslMethod::tls())?;
    ssl_builder.set_private_key(&p12.pkey)?;
    ssl_builder.set_certificate(&p12.cert)?;
    ssl_builder.set_ca_file(ca)?;
    if mtls {
        ssl_builder.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT)
    }
    Ok(ssl_builder)
}

pub fn client<P: AsRef<std::path::Path>>(
    ca: P,
    certificate_password: &str,
    certificate: P,
) -> Result<awc::Client, Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Read;
    let mut certificate_file = std::fs::File::open(certificate.as_ref()).map_err(|err| {
        log::warn!(
            "failed to read certificate: {}",
            certificate.as_ref().display()
        );
        err
    })?;
    let mut bytes = Vec::new();
    certificate_file.read_to_end(&mut bytes)?;
    let p12 = openssl::pkcs12::Pkcs12::from_der(&bytes)?.parse(certificate_password)?;
    let mut ssl_builder =
        openssl::ssl::SslConnector::builder(openssl::ssl::SslMethod::tls_client())?;
    ssl_builder.set_private_key(&p12.pkey)?;
    ssl_builder.set_certificate(&p12.cert)?;
    ssl_builder.set_ca_file(ca)?;
    Ok(awc::Client::build()
        .connector(awc::Connector::new().ssl(ssl_builder.build()).finish())
        .finish())
}
