use lazy_static::lazy_static;
use std::collections::HashSet;
use std::io;
use std::net::IpAddr;

lazy_static! {
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

pub fn own_ip(s: &IpAddr) -> bool {
    INTERFACE_IPS.contains(s)
        || match s {
            IpAddr::V4(v4) => v4.is_unspecified() || v4.is_broadcast(),
            IpAddr::V6(v6) => v6.is_unspecified(),
        }
}

// bincode serialize -> flate2 compress -> base64 encode
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

// base64 decode -> flate2 decompress -> bincode deserialize
pub fn bincode_gz_base64_dec<R: std::io::Read, D: serde::de::DeserializeOwned>(
    mut r: R,
) -> Result<D, std::io::Error> {
    let mut buf = Vec::new();
    r.read_to_end(&mut buf)?;
    let bytes = base64::decode(&buf).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    bincode::deserialize_from(flate2::read::GzDecoder::new(&bytes[..]))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
