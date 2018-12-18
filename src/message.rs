use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BallotID {
    pub b_id: usize,
    pub l_id: usize,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ballot<I> {
    pub b_id: BallotID,
    pub c_id: I,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PValue<I> {
    pub s_id: usize,
    pub b_id: BallotID,
    pub c_id: I,
}

pub type P1A = BallotID;

pub type P2A<I> = PValue<I>;

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P1B<I> {
    pub a_id: usize,
    pub b_id: BallotID,
    pub pvalues: Vec<PValue<I>>,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P2B {
    pub a_id: usize,
    pub b_id: BallotID,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal<I> {
    pub s_id: usize,
    pub c_id: I,
}
