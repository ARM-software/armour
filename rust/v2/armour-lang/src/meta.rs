use super::{
    expressions::{Error, Expr},
    externals::Call,
    labels,
    literals::Literal,
};
use actix::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::convert::{TryFrom, TryInto};

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    id: labels::Label,
    data: Vec<Vec<u8>>,
    labels: labels::Labels,
}

impl Meta {
    pub fn new(id: labels::Label) -> Self {
        Meta {
            id,
            data: Vec::new(),
            labels: BTreeSet::new(),
        }
    }
    fn set_id(&mut self, l: labels::Label) {
        self.id = l
    }
    fn id(&self) -> labels::Label {
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
    fn insert_label(&mut self, l: labels::Label) {
        self.labels.insert(l);
    }
    fn has_label(&self, label: &labels::Label) -> bool {
        self.labels.iter().any(|x| label.matches_with(x))
    }
    fn remove_label(&mut self, label: &labels::Label) {
        for l in self.labels.clone().iter() {
            if label.matches_with(l) {
                self.labels.remove(l);
            }
        }
    }
}

#[derive(PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct IngressEgress {
    ingress: Option<Meta>,
    egress: Option<Meta>,
}

impl Actor for IngressEgress {
    type Context = Context<Self>;
}

impl IngressEgress {
    pub fn new(ingress: Option<Meta>, egress: Option<Meta>) -> Self {
        IngressEgress { ingress, egress }
    }
}

impl TryFrom<&str> for Meta {
    type Error = ();
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse::<labels::Label>().map(Meta::new).map_err(|_| ())
    }
}

impl From<&str> for IngressEgress {
    fn from(s: &str) -> Self {
        IngressEgress::new(None, s.try_into().ok())
    }
}

impl From<(&str, &str)> for IngressEgress {
    fn from(s: (&str, &str)) -> Self {
        IngressEgress::new(s.0.try_into().ok(), s.0.try_into().ok())
    }
}

impl Handler<Call> for IngressEgress {
    type Result = Result<Expr, Error>;
    fn handle(&mut self, call: Call, _ctx: &mut Context<Self>) -> Self::Result {
        let (module, method, args) = call.split();
        if let Some(meta) = match module {
            "Ingress" => self.ingress.as_mut(),
            "Egress" => self.egress.as_mut(),
            _ => {
                return Err(Error::from(format!(
                    "eval, unknown module: {}",
                    call.path()
                )))
            }
        } {
            // NOTE: type checking prevents mutatation of "ingress" metadata
            match (method, args) {
                ("id", []) => Ok(Expr::some(meta.id().into())),
                ("data", []) => Ok(meta.data().into()),
                ("set_id", [Literal::Label(l)]) => {
                    meta.set_id(l.clone());
                    Ok(().into())
                }
                ("push", [Literal::Data(d)]) => {
                    meta.push_data(d);
                    Ok(().into())
                }
                ("pop", []) => Ok(meta.pop_data().into()),
                ("add_label", [Literal::Label(l)]) => {
                    meta.insert_label(l.clone());
                    Ok(().into())
                }
                ("remove_label", [Literal::Label(l)]) => {
                    meta.remove_label(&l);
                    Ok(().into())
                }
                ("has_label", [Literal::Label(l)]) => Ok(meta.has_label(l).into()),
                ("wipe", []) => {
                    self.egress = None;
                    Ok(().into())
                }
                _ => Err(Error::from(format!(
                    "eval, call: {}::{}: {:?}",
                    module, method, args
                ))),
            }
        } else {
            // metadata is absent
            match (method, args) {
                ("id", []) | ("pop", []) => Ok(Literal::none().into()),
                ("data", []) => Ok(Vec::<Vec<u8>>::new().into()),
                ("has_label", [Literal::Label(_l)]) => Ok(false.into()),
                _ => Ok(().into()),
            }
        }
    }
}

#[derive(Message)]
#[rtype("Result<Meta, Error>")]
pub struct Egress;

impl Handler<Egress> for IngressEgress {
    type Result = Result<Meta, Error>;
    fn handle(&mut self, _: Egress, _ctx: &mut Context<Self>) -> Self::Result {
        self.egress
            .as_ref()
            .cloned()
            .ok_or_else(|| Error::new("no egress metadata"))
    }
}
