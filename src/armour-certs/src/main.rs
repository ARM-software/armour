// Tool for generating Armour certificates
use clap::{crate_version, App, Arg};
use std::io::{Read, Write};

static ARMOUR_CN: &str = "Armour CA";
static ARMOUR_PREFIX: &str = "armour-";
static ARMOUR_DIR: &str = "certificates";
static ARMOUR_ALT_NAMES: &[AltName] = &[
    AltName::DNS("localhost"),
    AltName::IP(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))),
];

type Error = Box<dyn std::error::Error + Send + Sync>;

// generate a new public/private key pair
fn prime256v1_key() -> Result<openssl::pkey::PKey<openssl::pkey::Private>, Error> {
    let group = openssl::ec::EcGroup::from_curve_name(openssl::nid::Nid::X9_62_PRIME256V1)?;
    let key = openssl::ec::EcKey::generate(&group)?;
    Ok(openssl::pkey::PKey::from_ec_key(key)?)
}

// build name
fn build_x509_name(common_name: &str) -> Result<openssl::x509::X509Name, Error> {
    let mut x509_name = openssl::x509::X509NameBuilder::new()?;
    x509_name.append_entry_by_text("C", "UK")?;
    x509_name.append_entry_by_text("ST", "Cambs")?;
    x509_name.append_entry_by_text("O", "Armour")?;
    x509_name.append_entry_by_text("CN", common_name)?;
    Ok(x509_name.build())
}

#[derive(Clone, PartialEq)]
enum AltName<'a> {
    DNS(&'a str),
    IP(std::net::IpAddr),
}

impl<'a> std::fmt::Display for AltName<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AltName::DNS(domain) => write!(f, "DNS:{}", domain),
            AltName::IP(ip_addr) => write!(f, "IP:{}", ip_addr),
        }
    }
}

impl<'a> From<&'a str> for AltName<'a> {
    fn from(s: &'a str) -> Self {
        if let Ok(ip) = s.parse() {
            AltName::IP(ip)
        } else {
            AltName::DNS(s)
        }
    }
}

// build a certificate
fn build_x509(
    issuer: &openssl::x509::X509NameRef,
    subject: &openssl::x509::X509NameRef,
    public_key: &openssl::pkey::PKeyRef<openssl::pkey::Private>,
    sign_key: Option<&openssl::pkey::PKeyRef<openssl::pkey::Private>>,
    sign_cert: Option<&openssl::x509::X509Ref>,
    alt_names: &[AltName],
) -> Result<openssl::x509::X509, Error> {
    // random serial number
    let mut bn = openssl::bn::BigNum::new()?;
    bn.pseudo_rand(96, openssl::bn::MsbOption::MAYBE_ZERO, false)?;
    let asn1 = openssl::asn1::Asn1Integer::from_bn(&bn)?;
    // lasts one year
    let start = openssl::asn1::Asn1Time::days_from_now(0)?;
    let end = openssl::asn1::Asn1Time::days_from_now(365)?;
    // build certificate
    let mut x509 = openssl::x509::X509::builder()?;
    x509.set_not_before(&start)?;
    x509.set_not_after(&end)?;
    x509.set_version(0)?;
    x509.set_serial_number(&asn1)?;
    x509.set_issuer_name(&issuer)?;
    x509.set_subject_name(&subject)?;
    x509.set_pubkey(&public_key)?;
    if let Some(cert) = sign_cert {
        // CA signed certificate
        let extension = openssl::x509::X509Extension::new_nid(
            None,
            Some(&x509.x509v3_context(Some(cert), None)),
            openssl::nid::Nid::AUTHORITY_KEY_IDENTIFIER,
            "keyid, issuer",
        )?;
        x509.append_extension(extension)?;
        x509.append_extension(openssl::x509::X509Extension::new_nid(
            None,
            None,
            openssl::nid::Nid::BASIC_CONSTRAINTS,
            "CA:FALSE",
        )?)?;
        x509.append_extension(openssl::x509::X509Extension::new_nid(
            None,
            None,
            openssl::nid::Nid::KEY_USAGE,
            "digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment",
        )?)?
    } else {
        // self-signed (CA) certificate
        x509.append_extension(openssl::x509::X509Extension::new_nid(
            None,
            None,
            openssl::nid::Nid::BASIC_CONSTRAINTS,
            "CA:TRUE",
        )?)?;
        x509.append_extension(openssl::x509::X509Extension::new_nid(
            None,
            None,
            openssl::nid::Nid::KEY_USAGE,
            "keyCertSign, cRLSign",
        )?)?
    }
    if !alt_names.is_empty() {
        let value = alt_names
            .iter()
            .map(|alt_name| alt_name.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        x509.append_extension(openssl::x509::X509Extension::new_nid(
            None,
            None,
            openssl::nid::Nid::SUBJECT_ALT_NAME,
            &value,
        )?)?
    }
    x509.sign(
        &sign_key.unwrap_or(public_key),
        openssl::hash::MessageDigest::sha256(),
    )?;
    Ok(x509.build())
}

// write PEM file, together with public/private keys
fn write_pem<P: AsRef<std::path::Path>>(
    path: P,
    x509: &openssl::x509::X509,
    pkey: &openssl::pkey::PKey<openssl::pkey::Private>,
) -> Result<(), Error> {
    let mut file = std::fs::File::create(path.as_ref().with_extension("pem"))?;
    file.write_all(&x509.to_pem()?)?;

    let mut file = std::fs::File::create(path.as_ref())?;
    file.write_all(&pkey.private_key_to_pem_pkcs8()?)?;

    let mut file = std::fs::File::create(path.as_ref().with_extension("pub"))?;
    file.write_all(&pkey.public_key_to_pem()?)?;

    Ok(())
}

// write pkcs12 certificate
fn write_p12<P: AsRef<std::path::Path>>(
    path: P,
    p12_pass: &str,
    p12_name: &str,
    pkey: &openssl::pkey::PKey<openssl::pkey::Private>,
    x509: &openssl::x509::X509,
) -> Result<(), Error> {
    let p12 = openssl::pkcs12::Pkcs12::builder();
    let p12 = p12.build(p12_pass, p12_name, &pkey, &x509)?;
    let mut file = std::fs::File::create(path.as_ref().with_extension("p12"))?;
    file.write_all(&p12.to_der()?)?;
    Ok(())
}

// generate and sign a certificate, then write it using pkcs12
fn signed_x509<P: AsRef<std::path::Path>>(
    password: &str,
    dir: P,
    common_name: &str,
    issuer: &openssl::x509::X509NameRef,
    sign_key: &openssl::pkey::PKeyRef<openssl::pkey::Private>,
    sign_cert: &openssl::x509::X509Ref,
    alt_names: &[AltName],
) -> Result<openssl::x509::X509, Error> {
    let name = build_x509_name(common_name)?;
    let key = prime256v1_key()?;
    let x509 = build_x509(
        issuer,
        &name,
        &key,
        Some(sign_key),
        Some(sign_cert),
        alt_names,
    )?;
    let mut path = std::path::PathBuf::from(dir.as_ref());
    path.push(common_name);
    write_p12(path, password, common_name, &key, &x509)?;
    Ok(x509)
}

fn password(name: &str) -> Result<String, Error> {
    std::env::var("ARMOUR_PASS").or_else(|_| {
        std::env::var(format!("{}_PASS", name.to_uppercase())).or_else(|_| {
            rpassword::read_password_from_tty(Some(format!("{} password: ", name).as_str()))
                .map_err(|_| "failed to get password".into())
        })
    })
}

fn ip_dns_arg(name: &'static str) -> clap::Arg<'static, 'static> {
    Arg::with_name(name)
        .long(name)
        .required(false)
        .multiple(true)
        .takes_value(true)
        .value_name("IPv4 or DNS name")
}

fn main() -> Result<(), Error> {
    // passwords from env or shell
    //
    // dir (optional)
    //
    // control (multi - IP, DNS)
    // ctl (multi - IP, DNS)
    // host (multi - IP, DNS)
    // launch (multi - IP, DNS)

    let matches = App::new("armour-certs")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Armour Certificate Generation")
        .arg(
            Arg::with_name("directory")
                .short("d")
                .long("dir")
                .required(false)
                .takes_value(true)
                .help("Certificates directory"),
        )
        .arg(ip_dns_arg("control"))
        .arg(ip_dns_arg("host"))
        .get_matches();

    let dir = std::path::PathBuf::from(matches.value_of("directory").unwrap_or(ARMOUR_DIR));
    // create the directory if it doesn't already exist
    if !dir.is_dir() {
        if dir.exists() {
            // return Err(format!(r#"{} is not a directory"#, dir.display()));
            return Err("bad".into());
        } else {
            std::fs::create_dir(&dir)?
        }
    }

    // load CA certificate , or generate one if it doesn't already exist
    let ca_name = build_x509_name(ARMOUR_CN)?;
    let ca_key;
    let ca_x509;
    let mut ca_path = dir.clone();
    ca_path.push(format!("{}ca", ARMOUR_PREFIX));
    let pass = password("CA")?;
    if let Ok(mut file) = std::fs::File::open(&ca_path.with_extension("p12")) {
        // load pre-exiting CA certificate
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let p12 = openssl::pkcs12::Pkcs12::from_der(&bytes)?.parse(&pass)?;
        ca_key = p12.pkey;
        ca_x509 = p12.cert;
        println!("read {}.p12", ca_path.display())
    } else {
        // build CA cerificate
        ca_key = prime256v1_key()?;
        ca_x509 = build_x509(&ca_name, &ca_name, &ca_key, None, None, &[])?;
        write_pem(&ca_path, &ca_x509, &ca_key)?;
        write_p12(&ca_path, &pass, ARMOUR_CN, &ca_key, &ca_x509)?;
        println!("created {}", ca_path.display())
    }

    // build, sign and save certificates
    for name in &["control", "ctl", "host", "launch"] {
        let alt_names: Vec<AltName> = matches
            .values_of(name)
            .map(|names| names.map(|n| n.into()).collect())
            .unwrap_or_else(|| ARMOUR_ALT_NAMES.to_vec());
        let pass = password(name)?;
        let common_name = format!("{}{}", ARMOUR_PREFIX, name);
        signed_x509(
            &pass,
            &dir,
            &common_name,
            &ca_name,
            &ca_key,
            &ca_x509,
            &alt_names,
        )?;
        println!("created {}/{}.p12", dir.display(), common_name)
    }
    Ok(())
}
