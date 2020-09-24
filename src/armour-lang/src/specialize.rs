//Specialize control plan program to data plane one

use super::expressions::{Block, Error, Expr, Pattern};
use super::externals::{Call, ExternalActor};
use super::headers::Headers;
use super::labels::Label;
use super::lang::{Code, Program};
use super::literals::{Connection, HttpRequest, HttpResponse, Literal, Method, VecSet};
use super::meta::{Egress, IngressEgress, Meta};
use super::parser::{As, Infix, Iter, Pat, PolicyRegex, Prefix};s


impl CPExpr {

    //TODO do we need to write async code here -> not sure yet
    pub fn evaluate(self, env: Env) -> Result<Expr, self::Error> {
        Ok()     
    }
}