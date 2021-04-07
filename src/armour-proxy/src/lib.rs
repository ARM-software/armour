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

use actix::prelude::*;
use actix_web::web;
use armour_lang::{expressions, literals};

#[derive(Message)]
#[rtype("()")]
pub struct Stop;

pub mod http_policy;
pub mod http_proxy;
pub mod policy;
pub mod tcp_codec;
pub mod tcp_policy;
pub mod tcp_proxy;

/// Trait for converting rust types into Armour expressions
pub trait ToArmourExpression {
    fn to_expression(&self) -> expressions::DPExpr;
}

/// Convert an actix-web HttpRequest into an equivalent Armour language literal
impl ToArmourExpression for (&web::HttpRequest, &literals::DPConnection) {
    fn to_expression(&self) -> expressions::DPExpr {
        let (req, connection) = *self;
        literals::HttpRequest::new(
            req.method().as_str(),
            format!("{:?}", req.version()).as_str(),
            req.path(),
            req.query_string(),
            req.headers()
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_bytes()))
                .collect(),
            connection.clone(),
        )
        .into()
    }
}

/// Convert an actix-web HttpResponse into an equivalent Armour language literal
impl ToArmourExpression for (&web::HttpResponse, &literals::DPConnection) {
    fn to_expression(&self) -> expressions::DPExpr {
        let res = self.0;
        let head = res.head();
        literals::HttpResponse::new(
            format!("{:?}", head.version).as_str(),
            res.status().as_u16(),
            head.reason,
            res.headers()
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_bytes()))
                .collect(),
            self.1.clone(),
        )
        .into()
    }
}
