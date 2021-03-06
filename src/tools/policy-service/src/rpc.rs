/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use super::external_capnp::external;
use capnp::{capability::Promise, Error};
use std::fmt;

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Bool(bool),
    Data(Vec<u8>),
    Str(String),
    List(Vec<Literal>),
    Tuple(Vec<Literal>),
    Unit,
}

impl Literal {
    fn is_tuple(&self) -> bool {
        match self {
            Literal::Tuple(_) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Literal::Int(i) => write!(f, "Int({})", i),
            Literal::Float(d) => {
                if 8 < d.abs().log10() as usize {
                    write!(f, "Float({:e})", d)
                } else if (d.trunc() - *d).abs() < std::f64::EPSILON {
                    write!(f, "Float({:.1})", d)
                } else {
                    write!(f, "Float({})", d)
                }
            }
            Literal::Bool(b) => write!(f, "Bool({})", b),
            Literal::Data(d) => write!(f, "Data({})", String::from_utf8_lossy(d)),
            Literal::Str(s) => write!(f, r#"Str("{}")"#, s),
            Literal::List(lits) | Literal::Tuple(lits) => {
                let s = lits
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                if self.is_tuple() {
                    match lits.len() {
                        0 => write!(f, "None"),
                        1 => write!(f, "Some({})", s),
                        _ => write!(f, "({})", s),
                    }
                } else {
                    write!(f, "[{}]", s)
                }
            }
            Literal::Unit => write!(f, "Unit"),
        }
    }
}

fn build_value(mut v: external::value::Builder<'_>, lit: &Literal) -> Result<(), Error> {
    match lit {
        Literal::Bool(b) => v.set_bool(*b),
        Literal::Int(i) => v.set_int64(*i),
        Literal::Float(f) => v.set_float64(*f),
        Literal::Str(s) => v.set_text(s),
        Literal::Data(d) => v.set_data(d),
        Literal::Unit => v.set_unit(()),
        Literal::Tuple(ts) => {
            let mut tuple = v.init_tuple(ts.len() as u32);
            for (i, t) in ts.iter().enumerate() {
                build_value(tuple.reborrow().get(i as u32), t)?
            }
        }
        Literal::List(ts) => {
            let mut list = v.init_list(ts.len() as u32);
            for (i, t) in ts.iter().enumerate() {
                build_value(list.reborrow().get(i as u32), t)?
            }
        }
    }
    Ok(())
}

fn read_value(v: external::value::Reader<'_>) -> Result<Literal, capnp::Error> {
    use external::value::Which;
    match v.which() {
        Ok(Which::Bool(b)) => Ok(Literal::Bool(b)),
        Ok(Which::Int64(i)) => Ok(Literal::Int(i)),
        Ok(Which::Float64(f)) => Ok(Literal::Float(f)),
        Ok(Which::Text(t)) => Ok(Literal::Str(t?.to_string())),
        Ok(Which::Data(d)) => Ok(Literal::Data(d?.to_vec())),
        Ok(Which::Unit(_)) => Ok(Literal::Unit),
        Ok(Which::Tuple(ts)) => {
            let mut tuple = Vec::new();
            for t in ts? {
                tuple.push(read_value(t)?)
            }
            Ok(Literal::Tuple(tuple))
        }
        Ok(Which::List(ts)) => {
            let mut list = Vec::new();
            for t in ts? {
                list.push(read_value(t)?)
            }
            Ok(Literal::List(list))
        }
        Err(e) => Err(capnp::Error::from(e)),
    }
}

pub trait Dispatcher {
    fn dispatch(&mut self, name: &str, args: &[Literal]) -> Result<Literal, Error>;
    fn process_args(
        &self,
        args: capnp::struct_list::Reader<super::external_capnp::external::value::Owned>,
    ) -> Result<Vec<Literal>, Error> {
        let mut res = Vec::new();
        for arg in args {
            res.push(read_value(arg)?)
        }
        Ok(res)
    }
    fn log(&self, name: &str, args: &[Literal]) {
        log::info!("Call to method: {}", name);
        for (i, arg) in args.iter().enumerate() {
            log::info!("{}: {}", i, arg)
        }
    }
}

impl<D: Dispatcher> external::Server for D {
    fn call(
        &mut self,
        call: external::CallParams,
        mut result: external::CallResults,
    ) -> Promise<(), Error> {
        // process and print call
        let call = pry!(call.get());
        let name = pry!(call.get_name());
        let args = pry!(self.process_args(pry!(call.get_args())));
        // dispatch to method implementation and then set the result
        let res = result.get().init_result();
        if let Err(e) = build_value(res, &pry!(self.dispatch(name, &args))) {
            Promise::err(e)
        } else {
            Promise::ok(())
        }
    }
}
