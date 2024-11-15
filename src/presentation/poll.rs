use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    sync::Arc,
};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::NewPollMessage;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum VoteType {
    /// Someone can vote for one option with a value of 1
    SingleBinary { choice: String },
    /// Someone can vote for multiple choices with a value of 1
    MultipleBinary { choices: HashMap<String, bool> },
    /// Someone can vote for one option with a value between 0 and 255
    SingleValue { choice: String, value: u8 },
    /// Someone can vote for multiple choices with values between 0 and 255
    MultipleValue { choices: HashMap<String, u8> },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Vote {
    poll_name: String,
    vote_type: VoteType,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IdentifiedVote {
    pub identity: String,
    pub vote: Vote,
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
            warn!(
                "[{}] already voted for in [{}]",
                vote.identity, vote.vote.poll_name
            );
            return false;
        }

        // Ensure vote type is correct
        // is there a better way to do this?
        match (&self.vote_type, &vote.vote.vote_type) {
            (VoteType::SingleBinary { .. }, VoteType::SingleBinary { choice }) => {
                if !self.choices.contains(choice) {
                    warn!(
                        "[{}] tried to vote in poll with an invalid choice: [{}]",
                        vote.identity, choice
                    );
                    return false;
                }

                self.votes
                    .insert(vote.identity.clone(), vote.vote.vote_type.clone());
                // This is possible to deadlock if we ever hold other references.
                // So let's never do that.
                if self.totals.contains_key(choice) {
                    self.totals.alter(choice, |_, x| x + 1);
                } else {
                    self.totals.insert(choice.clone(), 1);
                }
                info!(
                    "[{}] voted for [{}] in [{}]",
                    vote.identity, choice, vote.vote.poll_name
                );
            }
            (VoteType::MultipleBinary { .. }, VoteType::MultipleBinary { choices }) => {
                for choice in choices.iter() {
                    if !self.choices.contains(choice.0) {
                        warn!(
                            "[{}] tried to vote in poll with an invalid choice: [{}]",
                            vote.identity, choice.0
                        );
                        return false;
                    }
                }

                self.votes
                    .insert(vote.identity, vote.vote.vote_type.clone());
                // This is possible to deadlock if we ever hold other references.
                // So let's never do that.
                for (choice, _) in choices.into_iter().filter(|(_, picked)| **picked) {
                    if self.totals.contains_key(choice) {
                        self.totals.alter(choice, |_, x| x + 1);
                    } else {
                        self.totals.insert(choice.clone(), 1);
                    }
                }
            }
            // TODO @obelisk: Implement the rest of these
            (VoteType::SingleValue { .. }, VoteType::SingleValue { .. }) => return false,
            (VoteType::MultipleValue { .. }, VoteType::MultipleValue { .. }) => return false,
            _ => {
                warn!(
                    "{} tried to vote for a poll with the wrong vote type: [{:?}] vs [{:?}]",
                    vote.identity, self.vote_type, vote.vote.vote_type
                );
                return false;
            }
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

    pub fn new_poll(&self, pole: NewPollMessage) -> Result<(), NewPollMessage> {
        if let Some(existing_pole) = self.polls.get(&pole.name) {
            let existing_pole = existing_pole.value().clone();
            Err(NewPollMessage {
                name: pole.name.clone(),
                options: existing_pole.choices.into_iter().collect(),
                vote_type: existing_pole.vote_type,
            })
        } else {
            self.polls
                .insert(pole.name, Poll::new(&pole.options, pole.vote_type));
            Ok(())
        }
    }

    pub fn vote_in_poll(&self, vote: IdentifiedVote) -> Result<(), String> {
        let vote_name = vote.vote.poll_name.clone();
        let identity = vote.identity.clone();
        match self
            .polls
            .get(&vote.vote.poll_name)
            .map(|poll| poll.vote(vote))
        {
            None => Err(format!("No poll with name {} exists", &vote_name)),
            Some(false) => Err(format!("{} could not vote in {}", identity, &vote_name)),
            Some(true) => Ok(()),
        }
    }

    pub fn get_poll_totals(&self, pole_name: &str) -> Option<HashMap<String, u64>> {
        self.polls.get(pole_name).map(|poll| {
            poll.value()
                .totals
                .iter()
                .map(|x| (x.key().to_string(), *x.value()))
                .collect()
        })
    }
}
