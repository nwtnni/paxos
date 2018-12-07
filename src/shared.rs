use std::sync::Arc;

use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub struct Shared(Arc<Mutex<State>>);

#[derive(Debug)]
struct State {

}
