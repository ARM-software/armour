use policy_service::rpc::Literal;
use rustyline::{completion, error::ReadlineError, hint, Editor};
use std::sync::{Arc, Mutex};

struct LoggerService(Arc<Mutex<i64>>);

impl policy_service::rpc::Dispatcher for LoggerService {
    fn dispatch(&mut self, name: &str, _args: &[Literal]) -> Result<Literal, capnp::Error> {
        match name {
            "log" => Ok(Literal::Unit),
            "state" => Ok(Literal::Int(*self.0.lock().unwrap())),
            _ => Err(capnp::Error::unimplemented(name.to_string())),
        }
    }
    fn log(&self) -> bool {
        true
    }
}

fn main() -> std::io::Result<()> {
    // enable logging
    std::env::set_var("RUST_LOG", "logger=info,policy_service=info,actix=debug");
    std::env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 {
        // start Actix system
        let sys = actix::System::new("logger");
        // start policy service actor
        let state = Arc::new(Mutex::new(0));
        let logger = LoggerService(state.clone());
        let socket = std::path::PathBuf::from(args[1].as_str());
        let policy_service = policy_service::start_policy_service(logger, socket)?;
        // issue commands based on user input
        std::thread::spawn(move || {
            let mut rl = Editor::new();
            rl.set_helper(Some(Helper::new()));
            if rl.load_history("logger.txt").is_err() {
                log::info!("no previous history");
            }
            loop {
                match rl.readline("logger:> ") {
                    Ok(cmd) => {
                        rl.add_history_entry(cmd.as_str());
                        match cmd.as_str() {
                            "" => (),
                            "suc" => {
                                let mut i = state.lock().unwrap();
                                *i += 1
                            }
                            "quit" => {
                                // remove socket
                                policy_service.do_send(policy_service::Quit);
                                break;
                            }
                            _ => log::warn!("unknown command"),
                        }
                    }
                    Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                        // remove socket
                        policy_service.do_send(policy_service::Quit);
                        break;
                    }
                    Err(err) => log::warn!("{}", err),
                }
            }
            rl.save_history("logger.txt")
                .expect("failed to save history")
        });
        sys.run()
    } else {
        println!("usage: {} <socket>", args[0]);
        Ok(())
    }
}

struct Helper(completion::FilenameCompleter, hint::HistoryHinter);

impl Helper {
    fn new() -> Self {
        Helper(completion::FilenameCompleter::new(), hint::HistoryHinter {})
    }
}

impl completion::Completer for Helper {
    type Candidate = completion::Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &rustyline::Context<'_>,
    ) -> Result<(usize, Vec<completion::Pair>), ReadlineError> {
        self.0.complete(line, pos, ctx)
    }
}
impl hint::Hinter for Helper {
    fn hint(&self, line: &str, pos: usize, ctx: &rustyline::Context<'_>) -> Option<String> {
        self.1.hint(line, pos, ctx)
    }
}

impl rustyline::highlight::Highlighter for Helper {}

impl rustyline::Helper for Helper {}
