use super::openai::Message;

/// Reduces message history before it is sent to a provider.
/// Implement this trait to add token counting, summarization, or a sliding-window strategy.
pub trait MessageCompactor: Send + Sync {
    /// Reduce `messages` so that the conversation fits within `max_tokens`.
    ///
    /// `max_tokens` is the upper bound on the number of tokens the compacted
    /// message list should represent. Implementations may drop older messages,
    /// summarize them, or apply any other strategy. Return the (possibly
    /// shortened) list that should be forwarded to the provider.
    fn compact(&self, messages: Vec<Message>, max_tokens: usize) -> Vec<Message>;
}

/// Default pass-through — messages are forwarded unchanged.
/// Replace `AppState::compactor` with a real implementation when token compaction is needed.
pub struct NoopCompactor;

impl MessageCompactor for NoopCompactor {
    fn compact(&self, messages: Vec<Message>, _max_tokens: usize) -> Vec<Message> {
        messages
    }
}
