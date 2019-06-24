#[macro_use]
extern crate log;

use armour_data_interface::ArmourPolicyRequest;
use std::path::PathBuf;

mod master {
    use actix::prelude::*;
    use armour_data_interface::{ArmourDataCodec, ArmourPolicyResponse, MasterArmourDataCodec};
    use futures::future;
    use tokio_codec::FramedRead;
    use tokio_io::{io::WriteHalf, AsyncRead};

    /// Armour policies, currently just Armour programs
    #[allow(dead_code)]
    pub struct ArmourPolicyMaster {
        uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, ArmourDataCodec>,
    }

    impl ArmourPolicyMaster {
        pub fn create_master<P: AsRef<std::path::Path>>(
            p: P,
        ) -> std::io::Result<Addr<ArmourPolicyMaster>> {
            tokio_uds::UnixStream::connect(p)
                .and_then(|stream| {
                    let addr = ArmourPolicyMaster::create(|ctx| {
                        let (r, w) = stream.split();
                        ctx.add_stream(FramedRead::new(r, MasterArmourDataCodec));
                        ArmourPolicyMaster {
                            uds_framed: actix::io::FramedWrite::new(w, ArmourDataCodec, ctx),
                        }
                    });
                    future::ok(addr)
                })
                .wait()
        }
    }

    impl actix::io::WriteHandler<std::io::Error> for ArmourPolicyMaster {}

    impl StreamHandler<ArmourPolicyResponse, std::io::Error> for ArmourPolicyMaster {
        fn handle(&mut self, msg: ArmourPolicyResponse, _ctx: &mut Context<Self>) {
            info!("get response: {:?}", msg)
        }
    }

    impl Actor for ArmourPolicyMaster {
        type Context = Context<Self>;
        fn started(&mut self, _ctx: &mut Self::Context) {
            info!("started Armour Policy Master")
        }
        fn stopped(&mut self, _ctx: &mut Self::Context) {
            info!("stopped Armour Policy Master")
        }
    }
}

fn main() -> std::io::Result<()> {
    // start Actix system
    let sys = actix::System::new("armour-data");

    // start up policy actor
    // (should possibly use actix::sync::SyncArbiter)
    let policy_master_addr = master::ArmourPolicyMaster::create_master("../armour-data/armour")?;

    std::thread::spawn(move || loop {
        let mut cmd = String::new();
        if std::io::stdin().read_line(&mut cmd).is_err() {
            println!("error");
            return;
        }
        // policy_master_addr.do_send(ArmourPolicyRequest::UpdateFromFile(PathBuf::from(
        //     cmd.trim_end_matches('\n'),
        // )));
    });

    sys.run()
}
