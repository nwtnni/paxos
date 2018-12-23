use serde_derive::{Deserialize, Serialize};

use crate::state;

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Clone(bound = ""), Debug(bound = ""), Hash(bound = ""), PartialEq(bound = ""), Eq(bound = ""))]
pub struct CommandID<C: state::Command> {
    pub c_id: C::ClientID, 
    pub l_id: C::LocalID,
}

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Clone, Debug)]
pub struct Command<C: state::Command>(C);

impl<C: state::Command> Eq for Command<C> {}

impl<C: state::Command> PartialEq for Command<C> {
    fn eq(&self, rhs: &Self) -> bool {
        self.0.client_id() == rhs.0.client_id() &&
        self.0.local_id() == rhs.0.local_id()
    }
}

impl<C: state::Command> std::hash::Hash for Command<C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.client_id().hash(state);
        self.0.local_id().hash(state);
    }
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BallotID {
    pub b_id: usize,
    pub l_id: usize,
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommanderID {
    pub b_id: BallotID,
    pub s_id: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Clone(bound = ""), Debug(bound = ""), Hash(bound = ""), PartialEq(bound = ""), Eq(bound = ""))]
pub struct Ballot<C: state::Command> {
    pub b_id: BallotID,
    pub c_id: CommandID<C>,
}

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Clone(bound = ""), Debug(bound = ""), Hash(bound = ""), PartialEq(bound = ""), Eq(bound = ""))]
pub struct PValue<C: state::Command> {
    pub s_id: usize,
    pub b_id: BallotID,
    pub c_id: CommandID<C>,
}

pub type P1A = BallotID;

pub type P2A<C> = PValue<C>;

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Clone(bound = ""), Debug(bound = ""), Hash(bound = ""), PartialEq(bound = ""), Eq(bound = ""))]
pub struct P1B<C: state::Command> {
    pub a_id: usize,
    pub b_id: BallotID,
    pub pvalues: Vec<PValue<C>>,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P2B {
    pub a_id: usize,
    pub b_id: BallotID,
}

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Clone(bound = ""), Debug(bound = ""), Hash(bound = ""), PartialEq(bound = ""), Eq(bound = ""))]
pub struct Proposal<C: state::Command> {
    pub s_id: usize,
    pub c_id: CommandID<C>,
}
