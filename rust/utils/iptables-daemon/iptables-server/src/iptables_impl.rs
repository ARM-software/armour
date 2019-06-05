use capnp::capability::Promise;

extern crate iptables_lib;
use iptables_lib::iptables_capnp::iptables;

extern crate iptables as iptbls; 

pub struct IptablesImpl;
impl iptables::Server for IptablesImpl {
    fn read(&mut self, params: iptables::ReadParams, mut results: iptables::ReadResults) -> 
        Promise<(), ::capnp::Error>
    { 
        let ipt = iptbls::new(false).unwrap();

        let a = pry!(params.get()).get_a();
        results.get().set_r(a);
        assert_eq!(ipt.new_chain("nat", "NEWCHAINNAME").unwrap(), true);
        assert_eq!(ipt.append("nat", "NEWCHAINNAME", "-j ACCEPT").unwrap(), true);
        assert_eq!(ipt.exists("nat", "NEWCHAINNAME", "-j ACCEPT").unwrap(), true);
        assert_eq!(ipt.delete("nat", "NEWCHAINNAME", "-j ACCEPT").unwrap(), true);
        assert_eq!(ipt.delete_chain("nat", "NEWCHAINNAME").unwrap(), true);
        Promise::ok(())
    }
   
    fn newchain(&mut self, params: iptables::NewchainParams, mut results: iptables::NewchainResults) -> 
        Promise<(), ::capnp::Error>
    { 
        let ipt = iptbls::new(false).unwrap();

        let args = pry!(params.get());
        let table = args.get_table().unwrap();
        let chain = args.get_chain().unwrap();

        let res = ipt.new_chain(table, chain).unwrap();
        results.get().set_result(res);
        Promise::ok(())
    }

    fn deletechain(&mut self, params: iptables::DeletechainParams, mut results: iptables::DeletechainResults) -> 
        Promise<(), ::capnp::Error>
    { 
        let ipt = iptbls::new(false).unwrap();

        let args = pry!(params.get());
        let table = args.get_table().unwrap();
        let chain = args.get_chain().unwrap();

        let res = ipt.delete_chain(table, chain).unwrap();
        results.get().set_result(res);
        Promise::ok(())
    }

    fn exists(&mut self, params: iptables::ExistsParams, mut results: iptables::ExistsResults) -> 
        Promise<(), ::capnp::Error>
    { 
        let ipt = iptbls::new(false).unwrap();

        let args = pry!(params.get());
        let table = args.get_table().unwrap();
        let chain = args.get_chain().unwrap();
        let rule = args.get_rule().unwrap();

        let res = ipt.exists(table, chain, rule).unwrap();
        results.get().set_result(res);
        Promise::ok(())
    }

    fn insert(&mut self, params: iptables::InsertParams, mut results: iptables::InsertResults) -> 
        Promise<(), ::capnp::Error>
    { 
        let ipt = iptbls::new(false).unwrap();

        let args = pry!(params.get());
        let table = args.get_table().unwrap();
        let chain = args.get_chain().unwrap();
        let rule = args.get_rule().unwrap();
        let pos :i32 = args.get_position().into();

        let res = ipt.insert(table, chain, rule, pos).unwrap();
        results.get().set_result(res);
        Promise::ok(())
    }

    fn append(&mut self, params: iptables::AppendParams, mut results: iptables::AppendResults) -> 
        Promise<(), ::capnp::Error>
    { 
        let ipt = iptbls::new(false).unwrap();

        let args = pry!(params.get());
        let table = args.get_table().unwrap();
        let chain = args.get_chain().unwrap();
        let rule = args.get_rule().unwrap();

        let res = ipt.append(table, chain, rule).unwrap();
        results.get().set_result(res);
        Promise::ok(())
    }

    fn delete(&mut self, params: iptables::DeleteParams, mut results: iptables::DeleteResults) -> 
        Promise<(), ::capnp::Error>
    { 
        let ipt = iptbls::new(false).unwrap();

        let args = pry!(params.get());
        let table = args.get_table().unwrap();
        let chain = args.get_chain().unwrap();
        let rule = args.get_rule().unwrap();

        let res = ipt.delete(table, chain, rule).unwrap();
        results.get().set_result(res);
        Promise::ok(())
    }
}
