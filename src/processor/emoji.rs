use crate::{
    OutgoingPresenterMessage, EmojiMessage, Presentation, Presenters, User
};

/// Called from the processor system. Only one processor should be called per user message
/// which is in a separate tokio task. Again this means we do not need to start tokio tasks
/// to unblock processesing of further user messages.
pub async fn handle_user_emoji(
    presentation: &Presentation,
    user: User,
    emoji_message: EmojiMessage,
    presenters: Presenters,
) {
    let slide_settings = presentation.slide_settings.read().await;
    // Check if the presentation has started
    let slide_settings = if let Some(ref s) = *slide_settings {
        s
    } else {
        error!(
            "{} sent a message but the presentation has not started",
            user.identity
        );
        return;
    };

    let emoji = &emoji_message.emoji;
    let identity = &user.identity;
    // Check that they are sending a valid emoji for the current slide
    if !slide_settings.emojis.contains(emoji) {
        error!("{identity} sent invalid {emoji} for current slide");
        return;
    }

    // Send the emojis to the presenters
    info!(
        "{identity} sent {emoji} to presentation {}",
        user.presentation
    );

    super::broadcast_to_presenters(OutgoingPresenterMessage::Emoji(emoji_message), presenters).await;
}
