use crate::{
    Presentation, Presenters, User, Vote, presentation::IdentifiedVote
};

/// Called from the processor system. Only one processor should be called per user message
/// which is in a separate tokio task. Again this means we do not need to start tokio tasks
/// to unblock processesing of further user messages.
pub async fn handle_user_vote(
    presentation: &Presentation,
    user: User,
    vote: Vote,
    _presenters: Presenters,
) {
    let identified_vote = IdentifiedVote {
        identity: user.identity.clone(),
        vote,
    };

    match presentation.get_poles().vote_in_pole(identified_vote) {
        Ok(_) => user.send_ignore_fail(crate::OutgoingUserMessage::Success(String::from("Vote recorded"))),
        Err(e) => user.send_ignore_fail(crate::OutgoingUserMessage::Error(e)),
    }
}