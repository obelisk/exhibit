use crate::{
    OutgoingPresenterMessage, EmojiMessage, Presentation, Presenters, User, Vote
};

/// Called from the processor system. Only one processor should be called per user message
/// which is in a separate tokio task. Again this means we do not need to start tokio tasks
/// to unblock processesing of further user messages.
pub async fn handle_user_vote(
    presentation: &Presentation,
    user: User,
    vote: Vote,
    presenters: Presenters,
) {

}