use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BallotID {
    pub b_id: usize,
    pub l_id: usize,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ballot<C> {
    pub b_id: BallotID,
    pub c_id: C,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PValue<C> {
    pub s_id: usize,
    pub b_id: BallotID,
    pub c_id: C,
}

pub type P1A = BallotID;

pub type P2A<C> = PValue<C>;

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P1B<C> {
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal<C> {
    pub s_id: usize,
    pub c_id: C,
}
