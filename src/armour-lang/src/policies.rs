/// policies
use super::{
    expressions,
    headers::THeaders,
    lang,
    literals::{self, TFlatLiteral},
    types::{self, Signature, FlatTyp, Typ, TFlatTyp, TTyp},
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
use std::marker::PhantomData;
use std::cmp::{Ord, Ordering, PartialOrd};

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
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct FnPolicies(pub BTreeMap<String, FnPolicy>);

impl FnPolicies {
    fn allow_all(names: &[String]) -> Self {
        FnPolicies(
            names
                .iter()
                .map(|name| (name.to_string(), FnPolicy::Allow))
                .collect(),
        )
    }
    fn allow_egress(names: &[String]) -> Self {
        FnPolicies(
            names
                .iter()
                .filter(|x| is_egress(x))
                .map(|name| (name.to_string(), FnPolicy::Allow))
                .collect(),
        )
    }
    fn allow_ingress(names: &[String]) -> Self {
        FnPolicies(
            names
                .iter()
                .filter(|x| is_ingress(x))
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
    fn deny_egress(names: &[String]) -> Self {
        FnPolicies(
            names
                .iter()
                .filter(|x| is_egress(x))
                .map(|name| (name.to_string(), FnPolicy::Deny))
                .collect(),
        )
    }
    fn deny_ingress(names: &[String]) -> Self {
        FnPolicies(
            names
                .iter()
                .filter(|x| is_ingress(x))
                .map(|name| (name.to_string(), FnPolicy::Deny))
                .collect(),
        )
    }
    fn is_allow_all(&self) -> bool {
        !self.0.is_empty() && self.0.values().all(|p| *p == FnPolicy::Allow)
    }
    fn is_allow_egress(&self) -> bool {
        !self.0.is_empty() && self.0.iter().all(|(k,p)| !is_egress(k) ||  *p == FnPolicy::Allow)
    }
    fn is_allow_ingress(&self) -> bool {
        !self.0.is_empty() && self.0.iter().all(|(k,p)| !is_ingress(k) ||  *p == FnPolicy::Allow)
    }
    fn is_deny_all(&self) -> bool {
        !self.0.is_empty() && self.0.values().all(|p| *p == FnPolicy::Deny)
    }
    fn is_deny_egress(&self) -> bool {
        !self.0.is_empty() && self.0.iter().all(|(k,p)| !is_egress(k) || *p == FnPolicy::Deny )
    }
    fn is_deny_ingress(&self) -> bool {
        !self.0.is_empty() && self.0.iter().all(|(k,p)| !is_ingress(k) || *p == FnPolicy::Deny )
    }
    
    pub fn merge(&self, other: &Self) -> Self{
        FnPolicies(self.0.clone().into_iter().chain(other.0.clone().into_iter()).collect())
    }
    pub fn set_args(&mut self, k:String, u: u8) {
        self.0.insert(k, FnPolicy::Args(u));
    }
}

// map from function name to list of permitted types
#[derive(Default)]
pub struct ProtocolPolicy<FlatTyp:TFlatTyp>(BTreeMap<String, Vec<Signature<FlatTyp>>>);
type DPProtocolPolicy = ProtocolPolicy<types::FlatTyp>;
type CPProtocolPolicy = ProtocolPolicy<types::CPFlatTyp>;

impl<FlatTyp:TFlatTyp> ProtocolPolicy<FlatTyp> {
    fn functions(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
    }
    pub fn insert(&mut self, name: &str, fn_policy: Vec<Signature<FlatTyp>>) {
        self.0.insert(name.to_string(), fn_policy);
    }
    fn insert_bool(&mut self, name: &str, args: Vec<Vec<Typ<FlatTyp>>>) {
        let sigs = args
            .into_iter()
            .map(|v| Signature::new(v, Typ::bool()))
            .collect();
        self.insert(name, sigs)
    }
    fn insert_unit(&mut self, name: &str, args: Vec<Vec<Typ<FlatTyp>>>) {
        let sigs = args
            .into_iter()
            .map(|v| Signature::new(v, Typ::unit()))
            .collect();
        self.insert(name, sigs)
    }
}

pub const ALLOW_REST_REQUEST: &str = "allow_rest_request";
pub const ALLOW_REST_RESPONSE: &str = "allow_rest_response";
pub const ALLOW_TCP_CONNECTION: &str = "allow_tcp_connection";
pub const ON_TCP_DISCONNECT: &str = "on_tcp_disconnect";

fn is_ingress(function: &String) -> bool {
    ALLOW_REST_REQUEST == function || ALLOW_TCP_CONNECTION == function
}

fn is_egress(function: &String) -> bool {
    ALLOW_REST_RESPONSE == function 
}

fn http_policy<FlatTyp:TFlatTyp>() -> ProtocolPolicy<FlatTyp> {      
    let mut policy = ProtocolPolicy::default();
    policy.insert_bool(
        ALLOW_REST_REQUEST,
        vec![
            vec![Typ::id(), Typ::id(), Typ::http_request(), Typ::data()],
            vec![Typ::http_request(), Typ::data()],
            vec![Typ::http_request()],
            Vec::new(),
        ],
    );
    policy.insert_bool(
        ALLOW_REST_RESPONSE,
        vec![
            vec![Typ::id(), Typ::id(), Typ::http_response(), Typ::data()],
            vec![Typ::FlatTyp(FlatTyp::http_response()), Typ::FlatTyp(FlatTyp::data())],
            vec![Typ::FlatTyp(FlatTyp::http_response())],
            Vec::new(),
        ],
    );
    policy
}

fn tcp_policy<FlatTyp:TFlatTyp>() -> ProtocolPolicy<FlatTyp> {
    let mut policy = ProtocolPolicy::default();
    policy.insert_bool(
        ALLOW_TCP_CONNECTION,
        vec![vec![Typ::FlatTyp(FlatTyp::connection())], Vec::new()],
    );
    policy.insert_unit(
        ON_TCP_DISCONNECT,
        vec![
            vec![Typ::FlatTyp(FlatTyp::connection()), Typ::FlatTyp(FlatTyp::i64()), Typ::FlatTyp(FlatTyp::i64())],
            vec![Typ::FlatTyp(FlatTyp::connection())],
            Vec::new(),
        ],
    );
    policy
}

lazy_static! {
    static ref CP_HTTP_POLICY: CPProtocolPolicy = http_policy();
    static ref HTTP_POLICY: DPProtocolPolicy = http_policy();
    static ref CP_TCP_POLICY: CPProtocolPolicy = tcp_policy();
    static ref TCP_POLICY: DPProtocolPolicy = tcp_policy();
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Protocol<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    HTTP,
    TCP,
    Phantom(PhantomData<(FlatTyp, FlatLiteral)>)
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> PartialOrd for Protocol<FlatTyp, FlatLiteral> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::HTTP, Self::TCP) => Some(Ordering::Less),
            (Self::TCP, Self::TCP) => Some(Ordering::Equal),
            (Self::HTTP, Self::HTTP) => Some(Ordering::Equal),
            (Self::TCP, Self::HTTP) => Some(Ordering::Greater),
            (Self::Phantom(_), _) => Some(Ordering::Less),
            (_, Self::Phantom(_)) => Some(Ordering::Greater),
        }
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Eq for Protocol<FlatTyp, FlatLiteral> {
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Ord for Protocol<FlatTyp, FlatLiteral> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.partial_cmp(other) {
            Some(x) => x,
            _ => unimplemented!(), //Should never happen, PhantomData

        }
    }
}

// Manual overloading since it is not stable yet
pub trait TProtocol<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    fn policy(p: &Protocol<FlatTyp, FlatLiteral>) -> &ProtocolPolicy<FlatTyp>;
}
pub type DPProtocol = Protocol<FlatTyp, literals::FlatLiteral>;
pub type CPProtocol = Protocol<types::CPFlatTyp, literals::CPFlatLiteral>;

impl From<CPProtocol> for DPProtocol {
    fn from(p: CPProtocol) -> Self {
        match p {
            CPProtocol::HTTP => Protocol::HTTP,
            CPProtocol::TCP => Protocol::TCP,
            CPProtocol::Phantom(_) => Protocol::Phantom(PhantomData)
        }
    } 
}
            
impl TProtocol<FlatTyp, Self> for literals::FlatLiteral {
    fn policy(p: &DPProtocol) -> &DPProtocolPolicy {
        match p {
            Protocol::HTTP => &*HTTP_POLICY,
            Protocol::TCP => &*TCP_POLICY,
            Protocol::Phantom(_) => unimplemented!(), 
        }
    }
}
impl TProtocol<types::CPFlatTyp, Self> for literals::CPFlatLiteral {
    fn policy(p: &CPProtocol) -> &CPProtocolPolicy {
        match p {
            Protocol::HTTP => &*CP_HTTP_POLICY,
            Protocol::TCP => &*CP_TCP_POLICY,
            Protocol::Phantom(_) => unimplemented!(), 
        }
    }
}
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Protocol<FlatTyp, FlatLiteral> {
    fn functions(&self) -> Vec<String> {
        self.policy().0.keys().cloned().collect()
    }
    fn policy(&self) -> &ProtocolPolicy<FlatTyp> {
        FlatLiteral::policy(self)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> fmt::Display for Protocol<FlatTyp, FlatLiteral> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Protocol::HTTP => write!(f, "http"),
            Protocol::TCP => write!(f, "tcp"),
            Protocol::Phantom(_) => unimplemented!()
        }
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> FromStr for Protocol<FlatTyp, FlatLiteral> {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(Protocol::TCP),
            "http" => Ok(Protocol::HTTP),
            _ => Err(format!("failed to parse protocol: {}", s)),
        }
    }
}

#[derive(PartialEq, Debug, Default, Serialize, Deserialize, Clone)]
pub struct Policy<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> {
    pub program: lang::Program<FlatTyp, FlatLiteral>,
    pub fn_policies: FnPolicies,
}
pub type DPPolicy = Policy<types::FlatTyp, literals::DPFlatLiteral>;
pub type GlobalPolicy = Policy<types::CPFlatTyp, literals::CPFlatLiteral>;

impl From<GlobalPolicy> for DPPolicy {
    fn from(gps: GlobalPolicy) -> Self {
        DPPolicy {
            program: lang::DPProgram::from(gps.program),
            fn_policies: gps.fn_policies
        }
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Policy<FlatTyp, FlatLiteral> {
    pub fn allow_all(p: Protocol<FlatTyp, FlatLiteral>) -> Self {
        let fn_policies = FnPolicies::allow_all(p.policy().functions().as_ref());
        Policy {
            program: lang::Program::default(),
            fn_policies,
        }
    }
    pub fn allow_egress(p: Protocol<FlatTyp, FlatLiteral>) -> Self {
        let fn_policies = FnPolicies::allow_egress(p.policy().functions().as_ref());
        Policy {
            program: lang::Program::default(),
            fn_policies,
        }
    }
    pub fn allow_ingress(p: Protocol<FlatTyp, FlatLiteral>) -> Self {
        let fn_policies = FnPolicies::allow_ingress(p.policy().functions().as_ref());
        Policy {
            program: lang::Program::default(),
            fn_policies,
        }
    }
    pub fn deny_all(p: Protocol<FlatTyp, FlatLiteral>) -> Self {
        let fn_policies = FnPolicies::deny_all(p.policy().functions().as_ref());
        Policy {
            program: lang::Program::default(),
            fn_policies,
        }
    }
    pub fn deny_egress(p: Protocol<FlatTyp, FlatLiteral>) -> Self {
        let fn_policies = FnPolicies::deny_egress(p.policy().functions().as_ref());
        Policy {
            program: lang::Program::default(),
            fn_policies,
        }
    }
    pub fn deny_ingress(p: Protocol<FlatTyp, FlatLiteral>) -> Self {
        let fn_policies = FnPolicies::deny_ingress(p.policy().functions().as_ref());
        Policy {
            program: lang::Program::default(),
            fn_policies,
        }
    }
    pub fn merge(&self, other: &Self) -> Self{
        Policy{
            program: self.program.merge(&other.program),
            fn_policies: self.fn_policies.merge(&other.fn_policies)
        }
    }
    pub fn get(&self, name: &str) -> Option<&FnPolicy> {
        self.fn_policies.0.get(name)
    }
    pub fn is_allow_all(&self) -> bool {
        self.fn_policies.is_allow_all()
    }
    pub fn is_allow_egress(&self) -> bool {
        self.fn_policies.is_allow_egress()
    }
    pub fn is_allow_ingress(&self) -> bool {
        self.fn_policies.is_allow_ingress()
    }
    pub fn is_deny_all(&self) -> bool {
        self.fn_policies.is_deny_all()
    }
    pub fn is_deny_egress(&self) -> bool {
        self.fn_policies.is_deny_egress()
    }
    pub fn is_deny_ingress(&self) -> bool {
        self.fn_policies.is_deny_ingress()
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
    fn type_check(function: &str, sig1: &Signature<FlatTyp>, sig2: &Signature<FlatTyp>) -> bool {
        let (args1, ty1) = sig1.split_as_ref();
        let (args2, ty2) = sig2.split_as_ref();
        Typ::type_check(function, vec![(None, ty1.clone())], vec![(None, ty2.clone())]).is_ok()
            && match (args1, args2) {
                (Some(a1), Some(a2)) => {
                    let a1 = a1.iter().map(|t| (None, t.clone())).collect();
                    let a2 = a2.iter().map(|t| (None, t.clone())).collect();
                    Typ::type_check(function, a1, a2).is_ok()
                }
                (Some(_), None) => false,
                (None, None) | (None, Some(_)) => true,
            }
    }
    fn from_program(
        program: lang::Program<FlatTyp, FlatLiteral>,
        proto_policy: &ProtocolPolicy<FlatTyp>,
    ) -> Result<Self, expressions::Error> {
        use std::convert::TryFrom;
        let mut fn_policies = FnPolicies::default();
        for (function, signatures) in proto_policy.0.iter() {
            if let Some(sig) = program.headers.typ(function) {
                if !signatures
                    .iter()
                    .any(|sig_typ| Self::type_check(function, &sig, sig_typ))
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

impl DPPolicy {
    fn from_bincode<R: std::io::Read>(r: R) -> Result<Self, std::io::Error> {
        armour_utils::bincode_gz_base64_dec(r)
    }
}
impl GlobalPolicy {
    fn from_bincode<R: std::io::Read>(r: R) -> Result<Self, std::io::Error> {
        armour_utils::bincode_gz_base64_dec(r)
    }
}

impl<FlatTyp: TFlatTyp, FlatLiteral: TFlatLiteral<FlatTyp>> fmt::Display for Policy<FlatTyp, FlatLiteral> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_allow_all() {
            write!(f, "allow all")
        } else if self.is_deny_all() {
            write!(f, "deny all")
        } else {
            if self.is_allow_egress() {
                write!(f, "allow egress")?;
            }
            if self.is_allow_ingress() {
                write!(f, "allow ingress")?;
            }
            if self.is_deny_ingress() {
                write!(f, "deny ingress")?;
            }
            if self.is_deny_ingress() {
                write!(f, "deny ingress")?;
            }

            writeln!(f, "[{}]", self.blake3())?;
            write!(f, "{}", self.program)
        }
    }
}

#[derive(PartialEq, Debug, Clone, Default)]
pub struct Policies<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>>(BTreeMap<Protocol<FlatTyp, FlatLiteral>, Policy<FlatTyp, FlatLiteral>>);
pub type DPPolicies = Policies<FlatTyp, literals::FlatLiteral>;
pub type GlobalPolicies = Policies<types::CPFlatTyp, literals::CPFlatLiteral>;

impl From<GlobalPolicies> for DPPolicies {
    fn from(gps: GlobalPolicies) -> Self {
        let mut pols = DPPolicies::new();
        for (p, pol)  in gps.policies() {
            pols.insert(DPProtocol::from(p.clone()), DPPolicy::from(pol.clone()));
        }
        pols
    }
}

impl From<DPPolicies> for literals::Policy {
    fn from(dppol: DPPolicies) -> Self {
           literals::Policy{pol: Box::new(dppol)} 
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Policies<FlatTyp, FlatLiteral> {
    pub fn new() -> Self {
        Policies(BTreeMap::new())  
    }
    pub fn insert(&mut self, p: Protocol<FlatTyp, FlatLiteral>, policy: Policy<FlatTyp, FlatLiteral>) {
        self.0.insert(p, policy);
    }

    pub fn merge(&self, other: &Self) -> Self{
        let mut new_pol = self.clone();

        for (k,v) in other.0.clone().into_iter() {
            match self.0.get(&k) {
                Some(x1) =>{ 
                    new_pol.insert(k, v.merge(x1))
                },
                None => new_pol.insert(k, v)
            }
        }

        new_pol
    }

    pub fn allow_all() -> Self {
        let mut policies = Policies::default();
        let tcp: Protocol<FlatTyp, FlatLiteral> = Protocol::TCP;
        let http: Protocol<FlatTyp, FlatLiteral> = Protocol::HTTP;
        policies
            .0
            .insert(Protocol::TCP, Policy::allow_all(tcp));
        policies
            .0
            .insert(Protocol::HTTP, Policy::allow_all(http));
        policies
    }
    pub fn allow_egress() -> Self {
        let mut policies = Policies::default();
        let tcp: Protocol<FlatTyp, FlatLiteral> = Protocol::TCP;
        let http: Protocol<FlatTyp, FlatLiteral> = Protocol::HTTP;
        policies
            .0
            .insert(Protocol::TCP, Policy::allow_egress(tcp));
        policies
            .0
            .insert(Protocol::HTTP, Policy::allow_egress(http));
        policies
    }
    pub fn allow_ingress() -> Self {
        let mut policies = Policies::default();
        let tcp: Protocol<FlatTyp, FlatLiteral> = Protocol::TCP;
        let http: Protocol<FlatTyp, FlatLiteral> = Protocol::HTTP;
        policies
            .0
            .insert(Protocol::TCP, Policy::allow_ingress(tcp));
        policies
            .0
            .insert(Protocol::HTTP, Policy::allow_ingress(http));
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
    pub fn deny_egress() -> Self {
        let mut policies = Policies::default();
        policies
            .0
            .insert(Protocol::TCP, Policy::deny_egress(Protocol::TCP));
        policies
            .0
            .insert(Protocol::HTTP, Policy::deny_egress(Protocol::HTTP));
        policies
    }
    pub fn deny_ingress() -> Self {
        let mut policies = Policies::default();
        policies
            .0
            .insert(Protocol::TCP, Policy::deny_ingress(Protocol::TCP));
        policies
            .0
            .insert(Protocol::HTTP, Policy::deny_ingress(Protocol::HTTP));
        policies
    }
    pub fn is_allow_all(&self) -> bool {
        self.0.values().all(|p| p.is_allow_all())
    }
    pub fn is_allow_egress(&self) -> bool {
        self.0.values().all(|p| p.is_allow_egress())
    }
    pub fn is_allow_ingress(&self) -> bool {
        self.0.values().all(|p| p.is_allow_ingress())
    }
    pub fn is_deny_all(&self) -> bool {
        self.0.values().all(|p| p.is_deny_all())
    }
    pub fn is_deny_egress(&self) -> bool {
        self.0.values().all(|p| p.is_deny_egress())
    }
    pub fn is_deny_ingress(&self) -> bool {
        self.0.values().all(|p| p.is_deny_ingress())
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn policy(&self, p: Protocol<FlatTyp, FlatLiteral>) -> Option<&Policy<FlatTyp, FlatLiteral>> {
        self.0.get(&p)
    }
    pub fn policies(&self) -> std::collections::btree_map::Iter<Protocol<FlatTyp, FlatLiteral>, Policy<FlatTyp, FlatLiteral>> {
        (&self.0).iter()
    }
    pub fn policies_mut(&mut self) -> std::collections::btree_map::IterMut<Protocol<FlatTyp, FlatLiteral>, Policy<FlatTyp, FlatLiteral>> {
        (&mut self.0).iter_mut()
    }

    fn inner_from(pre_prog: lang::PreProgram<FlatTyp, FlatLiteral>) -> Result<Self, expressions::Error> {
        let mut policies = Policies::default();
        let http : Protocol<FlatTyp, FlatLiteral> = Protocol::HTTP;
        let tcp : Protocol<FlatTyp, FlatLiteral> = Protocol::TCP;
        let http_prog = pre_prog.program(&http.functions());
        if !http_prog.is_empty() {
            policies.0.insert(
                Protocol::HTTP,
                Policy::from_program(http_prog, http.policy())?,
            );
        }
        let tcp_prog = pre_prog.program(&tcp.functions());
        if !tcp_prog.is_empty() {
            policies.0.insert(
                Protocol::TCP,
                Policy::from_program(tcp_prog, tcp.policy())?,
            );
        }
        Ok(policies)
    }

    pub fn from_buf(buf: &str) -> Result<Self, expressions::Error> {
        Self::inner_from(lang::PreProgram::from_buf(buf)?)
    }
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, expressions::Error> {
        Self::inner_from(lang::PreProgram::from_file(path)?)
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> fmt::Display for Policies<FlatTyp, FlatLiteral> {
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
        write!(f, "Phantom: ")?;
        if let Some(policy) = self.policy(Protocol::Phantom(PhantomData)) {
            writeln!(f, "{}", policy)?
        } else {
            writeln!(f, "-")?
        }        
        Ok(())
    }
}

impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Serialize for Policies<FlatTyp, FlatLiteral> {
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

struct DPPoliciesVisitor {} 
struct CPPoliciesVisitor {} 

impl<'de> Visitor<'de> for CPPoliciesVisitor {
    type Value = GlobalPolicies;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Policies")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map : GlobalPolicies = Policies::default();

        while let Some((proto, bincode_policy)) = access.next_entry::<CPProtocol, String>()? {
            let policy = GlobalPolicy::from_bincode(bincode_policy.as_bytes())
                .map_err(|_| serde::de::Error::custom("failed to read policy from bincode"))?;
            map.insert(proto, policy);
        }

        Ok(map)
    }
}
impl<'de> Visitor<'de> for DPPoliciesVisitor {
    type Value = DPPolicies;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Policies")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map : DPPolicies = Policies::default();

        while let Some((proto, bincode_policy)) = access.next_entry::<DPProtocol, String>()? {
            let policy = DPPolicy::from_bincode(bincode_policy.as_bytes())
                .map_err(|_| serde::de::Error::custom("failed to read policy from bincode"))?;
            map.insert(proto, policy);
        }

        Ok(map)
    }
}

impl<'de> Deserialize<'de> for DPPolicies {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(DPPoliciesVisitor{})
    }
}

impl<'de> Deserialize<'de> for GlobalPolicies {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(CPPoliciesVisitor{})
    }
}

pub const ONBOARDING_SERVICES: &str = "onboarding_policy";

pub type OnboardingPolicy = GlobalPolicy;
//#[derive(Serialize, Deserialize, Clone, Debug)]
//pub struct OnboardingPolicy {
//    //From ProtocolPolicy struct
//    pub name : String,//FIXME usefull ???
//    sig : CPSignature,//FIXME only one ??
//
//    //From Policy struct
//    program: CPProgram,
//    //fn_policies: FnPolicies,
//}

impl OnboardingPolicy {
    pub fn program<'a>(&'a self) -> &'a lang::CPProgram {
        &self.program
    }
    fn inner_from(pre_prog: lang::CPPreProgram) -> Result<Self, expressions::Error> {
        Ok(OnboardingPolicy {
            //name: ONBOARDING_SERVICES.to_string(),
            //sig: Signature::new(vec![CPTyp::onboardingData()], CPTyp::onboardingResult()),
            fn_policies: FnPolicies::default(),
            program: pre_prog.program(&vec![ONBOARDING_SERVICES.to_string()][..]),
        })
    }

    pub fn from_buf(buf: &str) -> Result<Self, expressions::Error> {
        Self::inner_from(lang::PreProgram::from_buf(buf)?)
    }
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, expressions::Error> {
        Self::inner_from(lang::PreProgram::from_file(path)?)
    }
    
    pub fn onboard_from(p: lang::CPProgram) -> Self {
        OnboardingPolicy {
            //name: ONBOARDING_SERVICES.to_string(),
            //sig: Signature::new(vec![CPTyp::onboardingData()], CPTyp::onboardingResult()),
            fn_policies: FnPolicies::default(),
            program: p,
        }
    }
}


//TODO create types : OnboardingData +  OnboardingResult
//FIXME : for now use protocoloPolicy instead of a dedicated OnboardingPolicy
//TODO: only one object Onboarding policiy is need at least for now
lazy_static! {
    static ref ONBOARDING_SERVICES_POLICY: OnboardingPolicy = OnboardingPolicy {
        //name: ONBOARDING_SERVICES.to_string(),
        //sig: Signature::new(vec![CPTyp::onboardingData()], CPTyp::onboardingResult()),
        fn_policies: FnPolicies::default(),
        program: lang::CPProgram::default(),
    };
}