use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};

pub use crate::endpoint::Endpoint as EP;
pub use crate::endpoint::EndpointRep as EPRep;

pub trait DataplanePolicy {
    type T;
    type E;

    // Data plane functions 
    fn validate(&self, source: impl EP, dest: impl EP) -> Result<Self::T, Self::E>;
}

pub trait ControlplanePolicy {
    type T;
    type E;

    // Control plane functions
    fn enable(&self, source: impl EP, dest: impl EP) -> Result<Self::T, Self::E>;
    fn disable(&self, source: impl EP, dest: impl EP) -> Result<Self::T, Self::E>;
}

pub trait ProxyPolicy : ControlplanePolicy + DataplanePolicy {}

// shared state that whitelists traffic to a destination port (to be replaced by full policy check)
// use of mutex could be an issue for efficiency/scaling!
type L3Policy = Arc<Mutex<HashMap<EPRep, HashSet<EPRep>>>>;

#[derive(Default)]
#[derive(Clone)]
pub struct PolicyStateL3 {
    pub state : L3Policy,
}

impl PolicyStateL3 {
    pub fn init() -> PolicyStateL3 {
        PolicyStateL3 {
            state: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub fn clone_state(&self) -> L3Policy {
        self.state.clone()
    }
    
    pub fn init_clone(p: L3Policy) -> PolicyStateL3 {
        PolicyStateL3 {
            state: p
        }
    }
}

impl DataplanePolicy for PolicyStateL3 {
    type T = bool;
    type E = (); // TODO: Fix error reporting
    
    fn validate(&self, source: impl EP, dest: impl EP) -> Result<Self::T, Self::E> {
        let policy = self.state.lock().unwrap();
        Ok(
            policy.contains_key(&source.rep()) &&
                policy.get(&source.rep()).unwrap().contains(&dest.rep())
        )
    }
}

impl ControlplanePolicy for PolicyStateL3 {
    type T = bool;
    type E = (); // TODO: Fix error reporting
    
    // Control plane functions
    fn enable(&self, source: impl EP, dest: impl EP) -> Result<Self::T, Self::E> {
        let mut policy = self.state.lock().unwrap();
        if policy.contains_key(&source.rep()) {
            policy.get_mut(&source.rep()).unwrap().insert(dest.rep());
        } else {
            let mut hs = HashSet::new();
            hs.insert(dest.rep());
            policy.insert(source.rep(), hs,);
        }
        Ok(true)
    }

    fn disable(&self, _source: impl EP, _dest: impl EP) -> Result<Self::T, Self::E> {
        Ok(true)
    }
}
