use std::{sync::Arc, collections::{HashSet, HashMap}, fmt::Display, f32::consts::E};

use dashmap::DashMap;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub enum VoteType {
    /// Someone can vote for one option with a value of 1
    SingleBinary{choice: String},
    /// Someone can vote for multiple choices with a value of 1
    MultipleBinary{choices: HashSet<String>},
    /// Someone can vote for one option with a value between 0 and 255
    SingleValue{choice: String, value: u8},
    /// Someone can vote for multiple choices with values between 0 and 255
    MultipleValue{choices: HashMap<String, u8>},
}

#[derive(Clone, Debug, Deserialize)]
pub struct Vote {
    poll_name: String,
    vote_type: VoteType,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IdentifiedVote {
    identity: String,
    vote: Vote,
}

#[derive(Clone)]
/// Structure that defines a poll and how its current state
/// Votes are a u8 so that way users can have up to 255 different
/// values to vote for while keeping sane tallying in totals which
/// is a u64.
pub struct Poll {
    votes: Arc<DashMap<String, VoteType>>,
    totals: Arc<DashMap<String, u64>>,
    choices: HashSet<String>,
    vote_type: VoteType,
}

impl Poll {
    pub fn new(choices: &[impl Display], vote_type: VoteType) -> Self {
        Self {
            votes: Arc::new(DashMap::new()),
            totals: Arc::new(DashMap::new()),
            choices: choices.into_iter().map(|x| x.to_string()).collect(),
            vote_type,
        }
    }

    pub fn vote(&self, vote: IdentifiedVote) -> bool {
        // If the user has already voted, don't let them do so again
        if self.votes.contains_key(vote.identity.as_str()) {
            return false;
        }

        // Ensure vote type is correct
        // is there a better way to do this?
        match (&self.vote_type, &vote.vote.vote_type) {
            (VoteType::SingleBinary{..}, VoteType::SingleBinary{choice}) => {
                if !self.choices.contains(choice) {
                    warn!("[{}] tried to vote in poll with an invalid choice: [{}]", vote.identity, choice);
                    return false;
                }

                self.votes.insert(vote.identity, vote.vote.vote_type.clone());
                // This is possible to deadlock if we ever hold other references.
                // So let's never do that.
                self.totals.alter(choice, |_, x| x + 1);
            },
            (VoteType::MultipleBinary{..}, VoteType::MultipleBinary{choices}) => {
                for choice in choices.iter() {
                    if !self.choices.contains(choice) {
                        warn!("[{}] tried to vote in poll with an invalid choice: [{}]", vote.identity, choice);
                        return false;
                    }
                }

                self.votes.insert(vote.identity, vote.vote.vote_type.clone());
                // This is possible to deadlock if we ever hold other references.
                // So let's never do that.
                for choice in choices {
                    self.totals.alter(choice, |_, x| x + 1);
                }
            },
            // TODO @obelisk: Implement the rest of these
            (VoteType::SingleValue{..}, VoteType::SingleValue{..}) => return false,
            (VoteType::MultipleValue{..}, VoteType::MultipleValue{..}) => return false,
            _ => {
                warn!("{} tried to vote for a poll with the wrong vote type: [{:?}] vs [{:?}]", vote.identity, self.vote_type, vote.vote.vote_type);
                return false
            },
        };

        
        true
    }
}


#[derive(Clone)]
pub struct Polls {
    polls: Arc<DashMap<String, Poll>>,
}

impl Polls {
    pub fn new() -> Self {
        Self {
            polls: Arc::new(DashMap::new()),
        }
    }

    pub fn new_poll(&self, name: impl Display, choices: &[impl Display], vote_type: VoteType) -> Result<(), String> {
        let name = name.to_string();
        if self.polls.contains_key(&name) {
            Err(format!("Poll with name {} already exists", name))
        } else {
            self.polls.insert(name, Poll::new(choices, vote_type));
            Ok(())
        }
    }

    pub fn vote_in_pole(&self, vote: IdentifiedVote) -> Result<(), String> {
        let vote_name = vote.vote.poll_name.clone();
        let identity = vote.identity.clone();
        match self.polls.get(&vote.vote.poll_name).map(|poll| {
            poll.vote(vote)
        }) {
            None => Err(format!("No poll with name {} exists", &vote_name)),
            Some(false) => Err(format!("{} could not vote in {}", identity, &vote_name)),
            Some(true) => Ok(()),
        }
    }
}