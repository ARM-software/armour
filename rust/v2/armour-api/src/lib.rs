/// Provides support for communication between Armour components
use byteorder::{BigEndian, ByteOrder};
use bytes::{Buf, BufMut, BytesMut};

pub mod control;
pub mod labels;
pub mod master;
pub mod proxy;

trait DeserializeDecoder<T: serde::de::DeserializeOwned, E: std::convert::From<std::io::Error>> {
    fn deserialize_decode(&mut self, src: &mut BytesMut) -> Result<Option<T>, E> {
        let size = {
            if src.len() < 2 {
                return Ok(None);
            }
            BigEndian::read_u16(src.as_ref()) as usize
        };
        if src.len() >= size + 2 {
            src.advance(2);
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
        dst.put_u16(msg_ref.len() as u16);
        dst.put(msg_ref);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::labels::{Label, LabelMap};
    use std::collections::HashSet;
    use std::convert::TryFrom;

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
    fn label_map() {
        let mut m = LabelMap::new();
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
    #[test]
    fn label_get() {
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
}
