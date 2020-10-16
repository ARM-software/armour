//Specialize control plane global policy to data plane local one 

use super::expressions::{Block, Error, Expr, Pattern};
use super::externals::{Call, ExternalActor};
use super::headers::Headers;
use super::labels::Label;
use super::lang::{Code, Program};
use super::literals::{self, Connection, HttpRequest, HttpResponse, CPID, CPLiteral, Method, VecSet};
use super::meta::{Egress, IngressEgress, Meta};
use super::parser::{As, Infix, Iter, Pat, PolicyRegex, Prefix};
use super::policies::{GlobalPolicies};


pub fn compile_ingress(global_pol: GlobalPolicies, function: &String, to: &CPID) -> CPLiteral {
    unimplemented!()
}

pub fn compile_egress(global_pol: GlobalPolicies, function: &String, to: &CPID) -> CPLiteral {
    unimplemented!()
}