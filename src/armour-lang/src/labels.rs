//! Label data type

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

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::slice::SliceIndex;
use std::str::FromStr;

thread_local!(static NODE_ANY: regex::Regex = regex::Regex::new("^<[[:alpha:]][[:alnum:]]*>$").unwrap());
thread_local!(static NODE_REC_ANY: regex::Regex = regex::Regex::new("^<<[[:alpha:]][[:alnum:]]*>>$").unwrap());
thread_local!(static NODE_STR: regex::Regex = regex::Regex::new("^[[:alnum:]]([ _+-]?[[:alnum:]])*$").unwrap());

/// Node element of [Label](struct.Label.html)
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Node {
    Any(String),
    RecAny(String),
    Str(String),
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Any(s) => {
                if s.is_empty() {
                    write!(f, "*")
                } else {
                    write!(f, "<{}>", s)
                }
            },
            Node::RecAny(s) => {
                if s.is_empty() {
                    write!(f, "**")
                } else {
                    write!(f, "<<{}>>", s)
                }
            }, 
            Node::Str(s) => write!(f, "{}", s),
        }
    }
}

impl FromStr for Node {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "*" {
            Ok(Node::Any(String::new()))
        } else if s == "**" {
            Ok(Node::RecAny(String::new()))   
        } else if NODE_ANY.with(|f| f.is_match(s)) {
            Ok(Node::Any(
                s.trim_start_matches('<').trim_end_matches('>').to_string(),
            ))
        } else if NODE_REC_ANY.with(|f| f.is_match(s)) {
            Ok(Node::RecAny(
                s.trim_start_matches('<').trim_end_matches('>').to_string(),
            ))
        } 
         else if NODE_STR.with(|f| f.is_match(s)) {
            Ok(Node::Str(s.to_string()))
        } else {
            Err(format!("bad label: {}", s))
        }
    }
}

enum MatchNode {
    Mismatch,
    Match,
    Map(String, Node),
    MapRec(String, Node),
}

/// Result of matching one label against another
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Match(BTreeMap<String, Label>);

impl fmt::Display for Match {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.0.iter();
        if let Some((key, node)) = iter.next() {
            write!(f, "<{}> |-> {}", key, node)?;
            for (key, node) in iter {
                write!(f, "; <{}> |-> {}", key, node)?
            }
        }
        Ok(())
    }
}

impl From<&Match> for Vec<(String, String)> {
    fn from(m: &Match) -> Self {
        m.0.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
}

impl Match {
    pub fn get_label<T: AsRef<str>>(&self, s: T) -> Option<&Label> {
        self.0.get(s.as_ref())
    }
    pub fn get<T: AsRef<str>>(&self, s: T) -> Option<String> {
        if let Some(l) = self.get_label(s) {
            Some(l.to_string())
        } else {
            None
        }
    }
}

impl Node {
    fn match_with(&self, l: &Node) -> MatchNode {
        match (self, l) {
            (Node::Str(x), Node::Str(y)) if x == y => MatchNode::Match,
            (Node::Str(_), _) => MatchNode::Mismatch,
            (Node::Any(x), _) if !x.is_empty() => MatchNode::Map(x.clone(), l.clone()),
            (Node::RecAny(x), _) if !x.is_empty() => MatchNode::MapRec(x.clone(), l.clone()),
            _ => MatchNode::Match,
        }
    }
    fn get_str(&self) -> Option<String> {
        if let Node::Str(s) = self {
            Some(s.to_string())
        } else {
            None
        }
    }
    fn get_any(&self) -> Option<String> {
        match self {
            Node::Any(s) if !s.is_empty() => Some(s.to_string()),
            Node::RecAny(s) if !s.is_empty() => Some(s.to_string()),
            _ => None,
        }
    }
}

/// Label type representing sequences of [Node](enum.Node.html) elements
#[derive(Clone, Default, PartialOrd, Ord)]
pub struct Label(Vec<Node>);

impl<'de> Deserialize<'de> for Label {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl Serialize for Label {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl FromStr for Label {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v: Result<Vec<Node>, Self::Err> = s.split("::").map(Node::from_str).collect();
        Ok(Label(v?))
    }
}

impl std::convert::From<&[Node]> for Label {
    fn from(v: &[Node]) -> Self {
        Label(v.to_vec())
    }
}

impl std::convert::From<&Node> for Label {
    fn from(n: &Node) -> Self {
        [n.to_owned()].as_ref().into()
    }
}

impl<T: AsRef<str>> std::convert::TryFrom<Vec<T>> for Label {
    type Error = String;

    fn try_from(s: Vec<T>) -> Result<Self, Self::Error> {
        let v: Result<Vec<Node>, Self::Error> =
            s.iter().map(|n| Node::from_str(n.as_ref())).collect();
        Ok(Label(v?))
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|node| node.to_string())
                .collect::<Vec<String>>()
                .join("::")
        )
    }
}

impl fmt::Debug for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl PartialEq for Label {
    fn eq(&self, other: &Self) -> bool {
        self.matches_with(other) && other.matches_with(self)
    }
}


impl Eq for Label {}

#[allow(clippy::len_without_is_empty)]
impl Label {
    // pub fn wild(&self) -> bool {
    //     self.0.iter().any(|n| n.0 == NodeType::Any)
    // }
    pub fn concat(l1: &Label, l2: &Label) -> Label {
        let mut tmp = l1.nodes().clone();
        tmp.extend(l2.nodes().clone().into_iter());
        Label(tmp)
    }

    pub fn prefix(&mut self, s:String) {
        self.0.insert(0, Node::Str(s));
    }
    pub fn push(&mut self, n:Node) {
        self.0.push(n);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn get<I>(&self, index: I) -> Option<&<I as SliceIndex<[Node]>>::Output>
    where
        I: SliceIndex<[Node]>,
    {
        self.0.get(index)
    }
    pub fn get_string(&self, index: usize) -> Option<String> {
        match self.get(index) {
            Some(Node::Str(s)) => Some(s.clone()),
            _ => None,
        }
    }
    fn nodes(&self) -> &Vec<Node> { &self.0 }    
    pub fn parts(&self) -> Option<Vec<String>> {
        self.0.iter().map(Node::get_str).collect()
    }
    pub fn vars(&self) -> Vec<String> {
        self.0.iter().filter_map(Node::get_any).collect()
    }
    pub fn matches_with(&self, l: &Label) -> bool {
        self.match_with(l).is_some()
    }
    pub fn match_with(&self, l: &Label) -> Option<Match> {
        //FIXME **/<<>> can only be define at the end of a pattern, ...::**
        let pattern = match self.0.last() {
            Some(Node::RecAny(y)) =>{
                let mut p = self.clone();
                for _ in self.0.len() .. l.0.len() {
                    p.0.push(Node::RecAny(y.clone()))    
                }
                p
            },
            _ => self.clone()
        };

        if pattern.0.len() != l.0.len() {
            return None;
        };
        let mut map = Match(BTreeMap::new());
        for (pat, node) in pattern.0.iter().zip(l.0.iter()) {
            match pat.match_with(node) {
                MatchNode::Map(l, r) => {
                    let mut ll = Label::default();
                    ll.push(r.clone());
                    if let Some(other) = map.0.insert(l, ll.clone()) {
                        if other != ll {
                            return None;
                        }
                    }
                },
                MatchNode::MapRec(l, r) => {
                    let mut ll = Label::default();
                    ll.push(r.clone());
                    match map.0.get_mut(&l) {
                        None => {map.0.insert(l, ll);},
                        Some(ref mut x) => x.push(r.clone())
                    };
                },
                MatchNode::Match => (),
                MatchNode::Mismatch => {
                    return None;
                }
            }
        }
        Some(map)
    }
    pub fn split_first(&self) -> Option<(Self, Option<Self>)> {
        self.0.split_first().map(|(first, rest)| {
            (
                first.into(),
                if rest.is_empty() {
                    None
                } else {
                    Some(rest.into())
                },
            )
        })
    }
    pub fn login_time(t: i64) -> Self {
        Label(vec![
            Node::Str("ControlPlane".to_string()),
            Node::Str("LoginTime".to_string()),
            Node::Str(t.to_string())
        ])
    }

    pub fn is_login_time(&self) -> bool {
        self.0.len() == 3 && 
            self.0[0] == Node::Str("ControlPlane".to_string()) && 
            self.0[1] == Node::Str("LoginTime".to_string())
    }

    pub fn get_login_time(&self) -> Option<i64> {
        if self.is_login_time() {
            match self.0[3] {
                Node::Str(ref s) => Some(s.parse::<i64>().unwrap()),
                _=> None
            }
        } else {
            None
        }
    }
}

pub type Labels = BTreeSet<Label>;

impl From<Label> for Labels {
    fn from(l: Label) -> Self {
        let mut labels = Labels::new();
        labels.insert(l);
        labels
    }
}

/* pub struct LabelMap<T>(Vec<(Label, T)>);

impl<T> Default for LabelMap<T> {
    fn default() -> Self {
        LabelMap(Vec::new())
    }
}

impl<T> LabelMap<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn lookup(&self, l: &Label) -> Vec<&T> {
        self.0
            .iter()
            .filter_map(|(k, v)| if k.matches_with(l) { Some(v) } else { None })
            .collect()
    }
    pub fn lookup_set(&self, l: &Label) -> HashSet<&T>
    where
        T: std::hash::Hash + Eq,
    {
        self.0
            .iter()
            .filter_map(|(k, v)| if k.matches_with(l) { Some(v) } else { None })
            .collect()
    }
    pub fn get(&self, l: &Label) -> Vec<&(Label, T)> {
        self.0.iter().filter(|(k, _)| l.matches_with(k)).collect()
    }
    pub fn remove(&mut self, l: &Label) -> Option<T> {
        if let Some(pos) = self.0.iter().position(|(k, _)| k == l) {
            Some(self.0.remove(pos).1)
        } else {
            None
        }
    }
    pub fn insert(&mut self, l: Label, v: T) -> Option<T> {
        // first remove entry for equivalent label
        let old = self.remove(&l);
        self.0.push((l, v));
        old
    }
}
 */
