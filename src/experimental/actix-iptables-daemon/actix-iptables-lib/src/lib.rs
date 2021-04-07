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

use actix::*;
use byteorder::{BigEndian, ByteOrder};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_io::codec::{Decoder, Encoder};
use tokio_io::{io::WriteHalf};
use tokio_uds;

#[macro_use]
extern crate log;
extern crate env_logger;

#[cfg(target_os="linux")]
extern crate iptables;


#[derive(Message)]
pub struct UdsConnect(pub tokio_uds::UnixStream);

#[derive(Serialize, Deserialize, Message, Clone, Debug)]
pub enum IptablesCommands {
    NewChain    {table: String, chain: String},
    DeleteChain {table: String, chain: String},
    Exists      {table: String, chain: String, rule: String},
    Insert      {table: String, chain: String, rule: String, possition: i32},
    Append      {table: String, chain: String, rule: String},
    Delete      {table: String, chain: String, rule: String},        
}

pub struct IptablesActor {
    pub ipt: iptables::IPTables,
    pub uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, IptablesCodec>,
}

impl Actor for IptablesActor { 
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        info!("Iptables actor started");
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("Iptables actor terminated");
    }
}

impl StreamHandler<IptablesCommands, std::io::Error> for IptablesActor {
    fn handle(&mut self, comm: IptablesCommands, _ctx: &mut Self::Context) {
        debug!("Handling Iptables command {:?}", comm);
        match &comm {
            IptablesCommands::NewChain {table, chain} => {
                let _res = self.ipt.new_chain(&table, &chain);
            } 
            IptablesCommands::DeleteChain {table, chain} => {
                let _res = self.ipt.delete_chain(&table, &chain);
            } 
            IptablesCommands::Exists {table, chain, rule} => {
                let _res = self.ipt.exists(&table, &chain, &rule);
            } 
            IptablesCommands::Insert {table, chain, rule, possition} => {
                let _res = self.ipt.insert(&table, &chain, &rule, *possition);
            }
            IptablesCommands::Append {table, chain, rule} => {
                let _res = self.ipt.append(&table, &chain, &rule);
            } 
            IptablesCommands::Delete {table, chain, rule} => {
                let _res = self.ipt.delete(&table, &chain, &rule);
            } 
        }
    }
}

impl actix::io::WriteHandler<std::io::Error> for IptablesActor {}

pub struct IptablesCodec;

impl DeserializeDecoder<IptablesCommands, std::io::Error> for IptablesCodec {}
impl SerializeEncoder<IptablesCommands, std::io::Error> for IptablesCodec {}

impl Decoder for IptablesCodec {
    type Item = IptablesCommands;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder for IptablesCodec {
    type Item = IptablesCommands;
    type Error = std::io::Error;
    fn encode(&mut self, msg: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}

trait DeserializeDecoder<T: serde::de::DeserializeOwned, E: std::convert::From<std::io::Error>> {
    fn deserialize_decode(&mut self, src: &mut BytesMut) -> Result<Option<T>, E> {
        let size = {
            if src.len() < 2 {
                return Ok(None);
            }
            BigEndian::read_u16(src.as_ref()) as usize
        };
        if src.len() >= size + 2 {
            src.split_to(2);
            let buf = src.split_to(size);
            Ok(Some(bincode::deserialize::<T>(&buf).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?))
        } else {
            Ok(None)
        }
    }
}

trait SerializeEncoder<T: serde::Serialize, E: std::convert::From<std::io::Error>> {
    fn serialize_encode(&mut self, msg: T, dst: &mut BytesMut) -> Result<(), E> {
        let msg = bincode::serialize(&msg).unwrap();
        let msg_ref: &[u8] = msg.as_ref();
        dst.reserve(msg_ref.len() + 2);
        dst.put_u16_be(msg_ref.len() as u16);
        dst.put(msg_ref);
        Ok(())
    }
}

// Client actor
pub struct IptablesClientActor {
    pub uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, IptablesCodec>,
}

impl Actor for IptablesClientActor { 
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("Iptables Client started");
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("Iptables Client terminated");
    }
}


// Simple command repeater
impl Handler<IptablesCommands> for IptablesClientActor {
    type Result = ();

    fn handle(&mut self, command: IptablesCommands, _ctx: &mut Context<Self>) {
        self.uds_framed.write(command);
    }
}

impl actix::io::WriteHandler<tokio::io::Error> for IptablesClientActor {}
