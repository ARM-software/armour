//! Communication interface between data plane host and proxy instances

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

use super::{host, DeserializeDecoder, SerializeEncoder};
use actix::prelude::*;
use armour_lang::{labels, policies};
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder};
use std::collections::{HashMap};

#[derive(Serialize, Deserialize, Clone)]
pub enum LabelOp {
    AddIp(Vec<(std::net::Ipv4Addr, labels::Labels)>),
    AddUri(Vec<(String, labels::Labels)>),
    RemoveIp(std::net::Ipv4Addr, Option<labels::Label>),
    RemoveUri(String, Option<labels::Label>),
    Clear,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HttpConfig {
    Port(u16),
    Ingress(u16, std::net::SocketAddrV4),
}

impl HttpConfig {
    pub fn port(&self) -> u16 {
        match self {
            HttpConfig::Port(p) => *p,
            HttpConfig::Ingress(p, _) => *p,
        }
    }
    pub fn ingress(&self) -> Option<std::net::SocketAddrV4> {
        match self {
            HttpConfig::Port(_p) => None,
            HttpConfig::Ingress(_p, socket) => Some(*socket),
        }
    }
}

/// Message to proxy instance
#[derive(Serialize, Deserialize, Message, Clone)]
#[rtype("()")]
pub enum PolicyRequest {
    CPOnboard(HashMap<std::net::IpAddr, labels::Labels>),
    Label(LabelOp),
    SetPolicy(policies::DPPolicies),
    Shutdown,
    StartHttp(HttpConfig),
    StartTcp(u16),
    Status,
    Stop(policies::DPProtocol),
    Timeout(u8),
}

/// Transport codec for Host to Proxy instance communication
pub struct PolicyCodec;

impl DeserializeDecoder<PolicyRequest, std::io::Error> for PolicyCodec {}
impl SerializeEncoder<host::PolicyResponse, std::io::Error> for PolicyCodec {}

impl Decoder for PolicyCodec {
    type Item = PolicyRequest;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder<host::PolicyResponse> for PolicyCodec {
    type Error = std::io::Error;
    fn encode(&mut self, msg: host::PolicyResponse, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}

