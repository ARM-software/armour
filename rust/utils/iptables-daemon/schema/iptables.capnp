@0xdb9472744c9442cf;

# Complete with other APIs as necessary: 
# https://github.com/yaa110/rust-iptables/
interface Iptables {
    newchain @0 (table :Text, chain :Text) -> (result :Bool);
    deletechain @1 (table :Text, chain :Text) -> (result :Bool);
    exists @2 (table :Text, chain :Text, rule :Text) -> (result :Bool);
    insert @3 (table :Text, chain :Text, rule :Text, position :UInt16) -> (result :Bool);
    append @4 (table :Text, chain :Text, rule :Text) -> (result :Bool);
    delete @5 (table :Text, chain :Text, rule :Text) -> (result :Bool);
}