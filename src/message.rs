use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BallotID {
    pub b_id: usize,
    pub l_id: usize,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ballot<O> {
    pub id: BallotID,
    pub op: O,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PValue<O> {
    pub slot: usize,
    pub id: BallotID,
    pub op: O,
}

pub type P1A = BallotID;

pub type P2A<O> = PValue<O>;

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P1B<O> {
    pub a_id: usize,
    pub b_id: BallotID,
    pub pvalues: Vec<PValue<O>>,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P2B {
    pub a_id: usize,
    pub b_id: BallotID,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision<O> {
    pub slot: usize,
    pub op: O,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out<O> {
    P1B(P1B<O>), 
    P2B(P2B),
}
