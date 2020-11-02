//! Armour policy language

#[macro_use]
extern crate enum_display_derive;

/// Cap'n Proto interface used by [externals](externals/index.html)
pub mod external_capnp {
    include!(concat!(env!("OUT_DIR"), "/external_capnp.rs"));
}

/// Language AST
pub mod expressions;
pub mod expressions_cp;
/// Make calls to external security services
///
/// For example, external services can be used for logging and session management
pub mod externals;
/// Record the types of built-in and user functions
pub mod headers;
/// Policy language interpreter
pub mod interpret;
/// Language interface
pub mod lang;
/// Lexer implemented using [nom](../nom/index.html)
pub mod lexer;
/// Armour primitive types
pub mod literals;
/// Metadata actor
pub mod meta;
/// Parser implemented using [nom](../nom/index.html)
pub mod parser;
/// Policies interface
pub mod policies;
/// Pretty-printer
pub mod pretty;
/// Type system
pub mod types;
 

pub mod labels;

#[cfg(test)]
mod tests {
    use super::labels::Label;

    #[test]
    fn match_with() {
        let pat: Label = "a::<a>::<a>::<b>".parse().unwrap();
        let lab: Label = "a::<b>::<b>::<b>".parse().unwrap();
        if let Some(m) = pat.match_with(&lab) {
            assert_eq!(m.get("a"), None);
            assert_eq!(m.get("b"), None)
        // println!("match: {}", m)
        } else {
            panic!("mismatch")
        }
    }
    #[test]
    fn get() {
        use super::labels::Node::*;
        assert_eq!(
            "a::b::c::d::*".parse::<Label>().unwrap().get(1..),
            Some(
                vec![
                    Str("b".to_string()),
                    Str("c".to_string()),
                    Str("d".to_string()),
                    Any(String::new())
                ]
                .as_slice()
            )
        );
        assert_eq!("a::b::c::d::*".parse::<Label>().unwrap().get(8..), None)
    }
    #[test]
    fn get_string() {
        assert_eq!(
            "a::b::c::d::*".parse::<Label>().unwrap().get_string(1),
            Some("b".to_string())
        )
    }
    #[test]
    fn parts() {
        assert_eq!(
            "a::b::c::d".parse::<Label>().unwrap().parts(),
            Some(vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string()
            ])
        )
    }
    #[test]
    fn parts_none() {
        assert_eq!("a::b::c::*".parse::<Label>().unwrap().parts(), None)
    }
    #[test]
    fn vars() {
        assert_eq!(
            "a::<b>::c::*::<d>".parse::<Label>().unwrap().vars(),
            vec!["b".to_string(), "d".to_string()]
        )
    }
    /*     #[test]
       fn label_map() {
           use std::collections::HashSet;
           use std::convert::TryFrom;
           let mut m = labels::LabelMap::new();
           let l1 = Label::try_from(vec!["a", "b"]).unwrap();
           let l2: Label = "a::b".parse().unwrap();
           let l3: Label = "a::*".parse().unwrap();
           let l4: Label = "*::b".parse().unwrap();
           let l5: Label = "c::d".parse().unwrap();
           let l6: Label = "*::*".parse().unwrap();
           m.insert(l1, 1);
           m.insert(l2.clone(), 12);
           m.insert(l3.clone(), 24);
           m.insert(l4.clone(), 12);
           m.insert(l5.clone(), 8);
           m.insert(l6.clone(), 9);
           assert_eq!(
               m.lookup_set(&l2),
               vec![9, 12, 24].iter().collect::<HashSet<&i32>>()
           );
           assert_eq!(m.get(&l3), vec![&(l2.clone(), 12), &(l3.clone(), 24)]);
           assert_eq!(m.get(&l4), vec![&(l2.clone(), 12), &(l4.clone(), 12)]);
           assert_eq!(
               m.get(&l6),
               vec![&(l2, 12), &(l3, 24), &(l4, 12), &(l5, 8), &(l6, 9)]
           )
       }
    */
}