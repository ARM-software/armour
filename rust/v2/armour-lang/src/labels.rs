//! Label data type

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::slice::SliceIndex;
use std::str::FromStr;

thread_local!(static NODE_ANY: regex::Regex = regex::Regex::new("^<[[:alpha:]][[:alnum:]]*>$").unwrap());
thread_local!(static NODE_STR: regex::Regex = regex::Regex::new("^[[:alpha:]]([ _+-]?[[:alnum:]])*$").unwrap());

/// Node element of [Label](struct.Label.html)
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Node {
    Any(String),
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
            }
            Node::Str(s) => write!(f, "{}", s),
        }
    }
}

impl FromStr for Node {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "*" {
            Ok(Node::Any(String::new()))
        } else if NODE_ANY.with(|f| f.is_match(s)) {
            Ok(Node::Any(
                s.trim_start_matches('<').trim_end_matches('>').to_string(),
            ))
        } else if NODE_STR.with(|f| f.is_match(s)) {
            Ok(Node::Str(s.to_string()))
        } else {
            Err("bad label")
        }
    }
}

enum MatchNode {
    Mismatch,
    Match,
    Map(String, Node),
}

/// Result of matching one label against another
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Match(BTreeMap<String, Node>);

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
    pub fn get_node<T: AsRef<str>>(&self, s: T) -> Option<&Node> {
        self.0.get(s.as_ref())
    }
    pub fn get<T: AsRef<str>>(&self, s: T) -> Option<String> {
        if let Some(Node::Str(v)) = self.get_node(s) {
            Some(v.to_string())
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
            _ => None,
        }
    }
}

/// Label type representing sequences of [Node](enum.Node.html) elements
#[derive(Serialize, Deserialize, Clone, PartialOrd, Ord)]
pub struct Label(Vec<Node>);

impl FromStr for Label {
    type Err = &'static str;

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

impl<T: AsRef<str>> std::convert::TryFrom<Vec<T>> for Label {
    type Error = &'static str;

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

impl Label {
    // pub fn wild(&self) -> bool {
    //     self.0.iter().any(|n| n.0 == NodeType::Any)
    // }
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
        if self.0.len() != l.0.len() {
            return None;
        };
        let mut map = Match(BTreeMap::new());
        for (pat, node) in self.0.iter().zip(l.0.iter()) {
            match pat.match_with(node) {
                MatchNode::Map(l, r) => {
                    if let Some(other) = map.0.insert(l, r.clone()) {
                        if other != r {
                            return None;
                        }
                    }
                }
                MatchNode::Match => (),
                MatchNode::Mismatch => {
                    return None;
                }
            }
        }
        Some(map)
    }
}

pub type Labels = BTreeSet<Label>;

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
