/// policies
use super::{
    expressions, lang,
    types::{Signature, Typ},
};
use lazy_static::lazy_static;
use serde::{
    de::{Deserializer, MapAccess, Visitor},
    ser::SerializeMap,
    Deserialize, Serialize,
};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

// policy for a function
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum FnPolicy {
    Allow,
    Deny,
    Args(u8),
}

impl Default for FnPolicy {
    fn default() -> Self {
        FnPolicy::Deny
    }
}

// map from function name to `FnPolicy`
#[derive(Serialize, Deserialize, Clone, Default)]
struct FnPolicies(BTreeMap<String, FnPolicy>);

impl FnPolicies {
    fn allow_all(names: &[String]) -> Self {
        FnPolicies(
            names
                .iter()
                .map(|name| (name.to_string(), FnPolicy::Allow))
                .collect(),
        )
    }
    fn deny_all(names: &[String]) -> Self {
        FnPolicies(
            names
                .iter()
                .map(|name| (name.to_string(), FnPolicy::Deny))
                .collect(),
        )
    }
    fn is_allow_all(&self) -> bool {
        self.0.values().all(|p| *p == FnPolicy::Allow)
    }
    fn is_deny_all(&self) -> bool {
        self.0.values().all(|p| *p == FnPolicy::Deny)
    }
}

// map from function name to list of permitted types
#[derive(Default)]
struct ProtocolPolicy(BTreeMap<String, Vec<Signature>>);

impl ProtocolPolicy {
    fn functions(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
    }
    fn insert(&mut self, name: &str, fn_policy: Vec<Signature>) {
        self.0.insert(name.to_string(), fn_policy);
    }
    fn insert_bool(&mut self, name: &str, args: Vec<Vec<Typ>>) {
        let sigs = args
            .into_iter()
            .map(|v| Signature::new(v, Typ::Bool))
            .collect();
        self.insert(name, sigs)
    }
    fn insert_unit(&mut self, name: &str, args: Vec<Vec<Typ>>) {
        let sigs = args
            .into_iter()
            .map(|v| Signature::new(v, Typ::Unit))
            .collect();
        self.insert(name, sigs)
    }
}

pub const ALLOW_REST_REQUEST: &str = "allow_rest_request";
pub const ALLOW_REST_RESPONSE: &str = "allow_rest_response";
pub const ALLOW_TCP_CONNECTION: &str = "allow_tcp_connection";
pub const ON_TCP_DISCONNECT: &str = "on_tcp_disconnect";

lazy_static! {
    static ref HTTP_POLICY: ProtocolPolicy = {
        let mut policy = ProtocolPolicy::default();
        policy.insert_bool(
            ALLOW_REST_REQUEST,
            vec![
                vec![Typ::HttpRequest, Typ::Data],
                vec![Typ::HttpRequest],
                Vec::new(),
            ],
        );
        policy.insert_bool(
            ALLOW_REST_RESPONSE,
            vec![
                vec![Typ::HttpResponse, Typ::Data],
                vec![Typ::HttpResponse],
                Vec::new(),
            ],
        );
        policy
    };
    static ref TCP_POLICY: ProtocolPolicy = {
        let mut policy = ProtocolPolicy::default();
        policy.insert_bool(
            ALLOW_TCP_CONNECTION,
            vec![vec![Typ::Connection], Vec::new()],
        );
        policy.insert_unit(
            ON_TCP_DISCONNECT,
            vec![
                vec![Typ::Connection, Typ::I64, Typ::I64],
                vec![Typ::Connection],
                Vec::new(),
            ],
        );
        policy
    };
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Protocol {
    HTTP,
    TCP,
}

impl Protocol {
    fn policy(&self) -> &ProtocolPolicy {
        match self {
            Protocol::HTTP => &*HTTP_POLICY,
            Protocol::TCP => &*TCP_POLICY,
        }
    }
    fn functions(&self) -> Vec<String> {
        self.policy().0.keys().cloned().collect()
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Protocol::HTTP => write!(f, "http"),
            Protocol::TCP => write!(f, "tcp"),
        }
    }
}

impl FromStr for Protocol {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(Protocol::TCP),
            "http" => Ok(Protocol::HTTP),
            _ => Err(format!("failed to parse protocol: {}", s)),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Policy {
    pub program: lang::Program,
    fn_policies: FnPolicies,
}

impl Policy {
    pub fn get(&self, name: &str) -> Option<&FnPolicy> {
        self.fn_policies.0.get(name)
    }
    pub fn allow_all(p: Protocol) -> Self {
        let fn_policies = FnPolicies::allow_all(p.policy().functions().as_ref());
        Policy {
            program: lang::Program::default(),
            fn_policies,
        }
    }
    pub fn deny_all(p: Protocol) -> Self {
        let fn_policies = FnPolicies::deny_all(p.policy().functions().as_ref());
        Policy {
            program: lang::Program::default(),
            fn_policies,
        }
    }
    pub fn is_allow_all(&self) -> bool {
        self.fn_policies.is_allow_all()
    }
    pub fn is_deny_all(&self) -> bool {
        self.fn_policies.is_deny_all()
    }
    fn blake3_hash(&self) -> Option<arrayvec::ArrayString<[u8; 64]>> {
        bincode::serialize(self)
            .map(|bytes| blake3::hash(&bytes).to_hex())
            .ok()
    }
    pub fn blake3(&self) -> String {
        self.blake3_hash()
            .map(|h| h.to_string())
            .unwrap_or_else(|| "<hash error>".to_string())
    }
    pub fn to_bincode(&self) -> Result<String, std::io::Error> {
        let mut buf = Vec::new();
        armour_utils::bincode_gz_base64_enc(&mut buf, self)?;
        Ok(std::str::from_utf8(&buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
            .to_string())
    }
    pub fn from_bincode<R: std::io::Read>(r: R) -> Result<Self, std::io::Error> {
        armour_utils::bincode_gz_base64_dec(r)
    }
    fn type_check(function: &str, sig1: &Signature, sig2: &Signature) -> bool {
        let (args1, ty1) = sig1.split_as_ref();
        let (args2, ty2) = sig2.split_as_ref();
        Typ::type_check(function, vec![(None, ty1)], vec![(None, ty2)]).is_ok()
            && match (args1, args2) {
                (Some(a1), Some(a2)) => {
                    let a1 = a1.iter().map(|t| (None, t)).collect();
                    let a2 = a2.iter().map(|t| (None, t)).collect();
                    Typ::type_check(function, a1, a2).is_ok()
                }
                (Some(_), None) => false,
                (None, None) | (None, Some(_)) => true,
            }
    }
    fn from_program(
        program: lang::Program,
        proto_policy: &ProtocolPolicy,
    ) -> Result<Self, expressions::Error> {
        use std::convert::TryFrom;
        let mut fn_policies = FnPolicies::default();
        for (function, signatures) in proto_policy.0.iter() {
            if let Some(sig) = program.headers.typ(function) {
                if !signatures
                    .iter()
                    .any(|sig_typ| Policy::type_check(function, &sig, sig_typ))
                {
                    let possible = signatures
                        .iter()
                        .map(|sig| sig.to_string())
                        .collect::<Vec<String>>()
                        .join("; ");
                    return Err(expressions::Error::new(format!(
                        r#"possible types for function "{}" are: {}"#,
                        function, possible
                    )));
                }
                if let Some(n) = sig.args().map(|v| u8::try_from(v.len()).ok()).flatten() {
                    fn_policies
                        .0
                        .insert(function.to_string(), FnPolicy::Args(n));
                } else {
                    log::warn!("failed to get arg count: {}", function);
                }
            } else {
                log::warn!("not present: {}", function);
                fn_policies.0.insert(function.to_string(), FnPolicy::Allow);
            }
        }
        Ok(Policy {
            program,
            fn_policies,
        })
    }
}

impl fmt::Display for Policy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_allow_all() {
            write!(f, "allow all")
        } else if self.is_deny_all() {
            write!(f, "deny all")
        } else {
            writeln!(f, "[{}]", self.blake3())?;
            write!(f, "{}", self.program)
        }
    }
}

#[derive(Clone, Default)]
pub struct Policies(BTreeMap<Protocol, Policy>);

impl Policies {
    fn insert(&mut self, p: Protocol, policy: Policy) {
        self.0.insert(p, policy);
    }
    pub fn allow_all() -> Self {
        let mut policies = Policies::default();
        policies
            .0
            .insert(Protocol::TCP, Policy::allow_all(Protocol::TCP));
        policies
            .0
            .insert(Protocol::HTTP, Policy::allow_all(Protocol::HTTP));
        policies
    }
    pub fn deny_all() -> Self {
        let mut policies = Policies::default();
        policies
            .0
            .insert(Protocol::TCP, Policy::deny_all(Protocol::TCP));
        policies
            .0
            .insert(Protocol::HTTP, Policy::deny_all(Protocol::HTTP));
        policies
    }
    pub fn is_allow_all(&self) -> bool {
        self.0.values().all(|p| p.is_allow_all())
    }
    pub fn is_deny_all(&self) -> bool {
        self.0.values().all(|p| p.is_deny_all())
    }
    pub fn policy(&self, p: Protocol) -> Option<&Policy> {
        self.0.get(&p)
    }
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, expressions::Error> {
        let pre_prog = lang::PreProgram::from_file(path)?;
        let mut policies = Policies::default();
        let http_prog = pre_prog.program(&Protocol::HTTP.functions());
        if !http_prog.is_empty() {
            policies.0.insert(
                Protocol::HTTP,
                Policy::from_program(http_prog, Protocol::HTTP.policy())?,
            );
        }
        let tcp_prog = pre_prog.program(&Protocol::TCP.functions());
        if !tcp_prog.is_empty() {
            policies.0.insert(
                Protocol::TCP,
                Policy::from_program(tcp_prog, Protocol::TCP.policy())?,
            );
        }
        Ok(policies)
    }
}

impl fmt::Display for Policies {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TCP: ")?;
        if let Some(policy) = self.policy(Protocol::TCP) {
            writeln!(f, "{}", policy)?
        } else {
            writeln!(f, "-")?
        }
        write!(f, "HTTP: ")?;
        if let Some(policy) = self.policy(Protocol::HTTP) {
            writeln!(f, "{}", policy)?
        } else {
            writeln!(f, "-")?
        }
        Ok(())
    }
}

impl Serialize for Policies {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (proto, policy) in self.0.iter() {
            let bincode_policy = policy
                .to_bincode()
                .map_err(|_| serde::ser::Error::custom("failed to convert policy to bincode"))?;
            map.serialize_entry(proto, &bincode_policy)?;
        }
        map.end()
    }
}

struct PoliciesVisitor;

impl<'de> Visitor<'de> for PoliciesVisitor {
    type Value = Policies;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Policies")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map = Policies::default();

        while let Some((proto, bincode_policy)) = access.next_entry::<Protocol, String>()? {
            let policy = Policy::from_bincode(bincode_policy.as_bytes())
                .map_err(|_| serde::de::Error::custom("failed to read policy from bincode"))?;
            map.insert(proto, policy);
        }

        Ok(map)
    }
}

impl<'de> Deserialize<'de> for Policies {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(PoliciesVisitor)
    }
}
