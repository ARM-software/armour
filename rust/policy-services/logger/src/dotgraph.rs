/*
  Support for exporting graphs using GraphViz "dot" format

  Author: Anthony Fox
*/
use dot::{Id, LabelText, Style};

struct ColourSchemeIterator<'a> {
    scheme: &'a str,
    current: u8,
    max: u8,
}

impl<'a> ColourSchemeIterator<'a> {
    fn new(scheme: &'a str, current: u8, max: u8) -> ColourSchemeIterator<'a> {
        ColourSchemeIterator {
            scheme,
            current,
            max,
        }
    }
    fn value(&self) -> String {
        self.current.to_string()
    }
}

impl<'a> Iterator for ColourSchemeIterator<'a> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        if self.current < self.max {
            self.current += 1;
            Some(format!("/{}/{}", self.scheme, self.value()))
        } else {
            None
        }
    }
}

pub struct ColourIterator<'a>(Vec<ColourSchemeIterator<'a>>);

impl<'a> ColourIterator<'a> {
    pub fn new(short: bool) -> ColourIterator<'a> {
        if short {
            ColourIterator(vec![ColourSchemeIterator::new("set312", 0, 12)])
        } else {
            ColourIterator(vec![
                ColourSchemeIterator::new("purples9", 1, 9),
                ColourSchemeIterator::new("greys9", 1, 9),
                ColourSchemeIterator::new("reds9", 1, 9),
                ColourSchemeIterator::new("greens9", 0, 9),
                ColourSchemeIterator::new("oranges9", 0, 9),
                ColourSchemeIterator::new("blues9", 0, 9),
            ])
        }
    }
}

impl<'a> Iterator for ColourIterator<'a> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        match self.0.pop() {
            Some(mut iterator) => match iterator.next() {
                Some(s) => {
                    self.0.push(iterator);
                    self.0.rotate_left(1);
                    Some(s)
                }
                None => self.next(),
            },
            None => None,
        }
    }
}

// user node type
// pub type Node<'a> = (&'a str, &'a str);
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node<'a> {
    pub label: &'a str,
    caption: &'a str,
    shape: &'a str,
    colour: String,
    bold: bool,
}

impl<'a> Node<'a> {
    pub fn new(
        label: &'a str,
        caption: &'a str,
        shape: &'a str,
        colour: String,
        bold: bool,
    ) -> Node<'a> {
        Node {
            label,
            caption,
            shape,
            colour,
            bold,
        }
    }
}

// user edge type
pub type Edge<'a> = (usize, usize, String);

// internal node type
#[derive(Clone)]
pub struct Nd<'a> {
    id: usize,
    node: Node<'a>,
}

impl<'a> Nd<'a> {
    fn new(id: usize, node: Node<'a>) -> Nd<'a> {
        Nd { id, node }
    }
}

// internal edge type
type Ed<'a> = (Nd<'a>, Nd<'a>, &'a str);

pub struct DotGraph<'a> {
    pub name: &'a str,
    pub nodes: Vec<Node<'a>>,
    pub edges: Vec<Edge<'a>>,
    pub edge_colour: &'a str,
    pub node_label_size: u16,
    pub caption_label_size: u16,
    pub edge_label_size: u16,
    pub font: &'a str,
}

impl<'a> DotGraph<'a> {
    fn dot_name(&self) -> String {
        self.name
            .to_string()
            .chars()
            .map(|c| {
                if c.is_whitespace() || !c.is_alphanumeric() {
                    '_'
                } else {
                    c
                }
            })
            .collect()
    }
}

impl<'a> dot::Labeller<'a, Nd<'a>, Ed<'a>> for DotGraph<'a> {
    fn graph_id(&'a self) -> dot::Id<'a> {
        Id::new(self.dot_name()).unwrap()
    }
    fn node_id(&'a self, n: &Nd<'a>) -> dot::Id<'a> {
        Id::new(format!("N{}", n.id)).unwrap()
    }
    fn node_label<'b>(&'b self, n: &Nd<'b>) -> dot::LabelText<'b> {
        match (n.node.bold, n.node.caption == "") {
            (true, true) => LabelText::html(format!(
                r#"<font face="{}" point-size="{}">&#x00AB; <b>{}</b> &#x00BB;</font>"#,
                self.font, self.node_label_size, n.node.label
            )),
            (false, true) => LabelText::html(format!(
                r#"<font face="{}" point-size="{}">{}</font>"#,
                self.font, self.node_label_size, n.node.label
            )),
            (true, false) => LabelText::html(format!(
                r#"<font face="{}" point-size="{}">&#x00AB; <b>{}</b> &#x00BB;</font><br/><font face="{}" point-size="{}">({})</font>"#,
                self.font, self.node_label_size, n.node.label, self.font, self.caption_label_size, n.node.caption
            )),
            (false, false) => LabelText::html(format!(
                r#"<font face="{}" point-size="{}">{}</font><br/><font face="{}" point-size="{}">({})</font>"#,
                self.font, self.node_label_size, n.node.label, self.font, self.caption_label_size, n.node.caption
            )),
        }
    }
    fn node_shape<'b>(&'b self, n: &Nd<'b>) -> Option<dot::LabelText<'b>> {
        Some(LabelText::label(n.node.shape))
    }
    fn node_color<'b>(&'b self, n: &Nd<'b>) -> Option<dot::LabelText<'b>> {
        Some(LabelText::label(n.node.colour.clone()))
    }
    fn node_style(&self, n: &Nd) -> dot::Style {
        if n.node.colour != "black" {
            Style::Filled
        } else {
            Style::None
        }
    }
    fn edge_label<'b>(&'b self, e: &Ed<'b>) -> dot::LabelText<'b> {
        LabelText::html(format!(
            r#"<font face="{}" color="{}" point-size="{}"> {}</font>"#,
            self.font, self.edge_colour, self.edge_label_size, e.2
        ))
    }
    // fn edge_style(&self, e: &Ed) -> dot::Style {
    //     Style::Solid
    // }
    fn edge_color<'b>(&'b self, _e: &Ed<'b>) -> Option<dot::LabelText<'b>> {
        Some(LabelText::label(self.edge_colour))
    }
}

impl<'a> dot::GraphWalk<'a, Nd<'a>, Ed<'a>> for DotGraph<'a> {
    fn nodes(&'a self) -> dot::Nodes<'a, Nd<'a>> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(id, node)| Nd::new(id, node.clone()))
            .collect()
    }
    fn edges(&'a self) -> dot::Edges<'a, Ed<'a>> {
        self.edges
            .iter()
            .map(|&(i, j, ref s)| {
                (
                    Nd::new(i, self.nodes[i].clone()),
                    Nd::new(j, self.nodes[j].clone()),
                    s.as_str(),
                )
            })
            .collect()
    }
    fn source(&self, e: &Ed<'a>) -> Nd<'a> {
        e.0.clone()
    }
    fn target(&self, e: &Ed<'a>) -> Nd<'a> {
        e.1.clone()
    }
}
