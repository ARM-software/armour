use super::{
    expressions::{Error, Expr},
    externals::Call,
    labels::{Label, Labels},
    literals::{Literal, TFlatLiteral},
    types::{TFlatTyp},
};
use actix::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::convert::TryFrom;

#[derive(PartialEq, Debug, Default, Clone, Serialize, Deserialize)]
pub struct Meta {
    id: Option<Label>,
    data: Vec<Vec<u8>>,
    labels: Labels,
}

impl Meta {
    pub fn new(id: Label) -> Self {
        Meta {
            id: Some(id),
            data: Vec::new(),
            labels: BTreeSet::new(),
        }
    }
    fn set_id(&mut self, l: Label) {
        self.id = Some(l)
    }
    fn id(&self) -> Option<Label> {
        self.id.clone()
    }
    fn push_data(&mut self, d: &[u8]) {
        self.data.push(d.to_vec())
    }
    fn pop_data(&mut self) -> Option<Vec<u8>> {
        self.data.pop()
    }
    fn data(&self) -> Vec<Vec<u8>> {
        self.data.clone()
    }
    fn insert_label(&mut self, l: Label) {
        self.labels.insert(l);
    }
    fn has_label(&self, label: &Label) -> bool {
        self.labels.iter().any(|x| label.matches_with(x))
    }
    fn remove_label(&mut self, label: &Label) {
        for l in self.labels.clone().iter() {
            if label.matches_with(l) {
                self.labels.remove(l);
            }
        }
    }
    fn wipe(&mut self) {
        self.id = None;
        self.data.clear();
        self.labels.clear();
    }
    fn is_empty(&self) -> bool {
        self.id.is_none() && self.data.is_empty() && self.labels.is_empty()
    }
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct IngressEgress {
    ingress: Meta,
    egress: Meta,
    egress_id: Label,
}

impl Actor for IngressEgress {
    type Context = Context<Self>;
}

impl Default for IngressEgress {
    fn default() -> Self {
        IngressEgress {
            ingress: Meta::default(),
            egress: Meta::default(),
            egress_id: "egress".parse().unwrap(),
        }
    }
}

impl IngressEgress {
    pub fn new(ingress: Option<Meta>, egress_id: Label) -> Self {
        IngressEgress {
            ingress: ingress.unwrap_or_default(),
            egress: Meta::default(),
            egress_id,
        }
    }
}

impl TryFrom<&str> for Meta {
    type Error = ();
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse::<Label>().map(Meta::new).map_err(|_| ())
    }
}

impl<FlatTyp, FlatLiteral> Handler<Call<FlatTyp, FlatLiteral>> for IngressEgress
where
    FlatTyp: TFlatTyp,
    FlatLiteral: TFlatLiteral<FlatTyp>
{
    type Result = Result<Expr<FlatTyp, FlatLiteral>, Error>;
    fn handle(&mut self, call: Call<FlatTyp, FlatLiteral>, _ctx: &mut Context<Self>) -> Self::Result {
        let (module, method, args) = call.split();
        let meta = match module {
            "Ingress" => &mut self.ingress,
            "Egress" => &mut self.egress,
            _ => {
                return Err(Error::from(format!(
                    "eval, unknown module: {}",
                    call.path()
                )))
            }
        };
        // NOTE: type checking prevents mutatation of "ingress" metadata
        match (method, args) {
            ("id", []) => Ok(meta.id().into()),
            ("data", []) => Ok(meta.data().into()),
            ("set_id", []) => {
                meta.set_id(self.egress_id.clone());
                Ok(().into())
            }
            ("push", [Literal::FlatLiteral(fl)]) if fl.is_data() => {
                meta.push_data(&fl.get_data());
                Ok(().into())
            }
            ("pop", []) => Ok(meta.pop_data().into()),
            ("add_label", [Literal::FlatLiteral(fl)]) if fl.is_label()=> {
                meta.insert_label(fl.get_label().clone());
                Ok(().into())
            }
            ("remove_label", [Literal::FlatLiteral(fl)]) if fl.is_label()=> {
                meta.remove_label(&fl.get_label());
                Ok(().into())
            }
            ("has_label", [Literal::FlatLiteral(fl)]) if fl.is_label() => Ok(meta.has_label(fl.get_label()).into()),
            ("wipe", []) => {
                meta.wipe();
                Ok(().into())
            }
            _ => Err(Error::from(format!(
                "eval, call: {}::{}: {:?}",
                module, method, args
            ))),
        }
    }
}

#[derive(Message)]
#[rtype("Result<Meta, Error>")]
pub struct Egress;

impl Handler<Egress> for IngressEgress {
    type Result = Result<Meta, Error>;
    fn handle(&mut self, _: Egress, _ctx: &mut Context<Self>) -> Self::Result {
        if self.egress.is_empty() {
            Err("empty egress".to_string().into())
        } else {
            let mut egress = self.egress.clone();
            egress.set_id(self.egress_id.clone());
            Ok(egress)
        }
    }
}
