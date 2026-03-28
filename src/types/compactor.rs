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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::openai::Message;

    fn msg(role: &str, content: &str) -> Message {
        Message { role: role.into(), content: content.into() }
    }

    #[test]
    fn noop_returns_messages_unchanged() {
        let messages = vec![msg("user", "hello"), msg("assistant", "hi")];
        let result = NoopCompactor.compact(messages, 1000);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[0].content, "hello");
        assert_eq!(result[1].role, "assistant");
        assert_eq!(result[1].content, "hi");
    }

    #[test]
    fn noop_ignores_max_tokens_constraint() {
        // Even with max_tokens = 1 the noop compactor returns everything unchanged.
        let messages = vec![msg("user", "a very long message that exceeds one token")];
        let result = NoopCompactor.compact(messages, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "a very long message that exceeds one token");
    }

    #[test]
    fn noop_with_empty_input_returns_empty() {
        let result = NoopCompactor.compact(vec![], usize::MAX);
        assert!(result.is_empty());
    }

    #[test]
    fn noop_preserves_order() {
        let messages: Vec<Message> = (0..5)
            .map(|i| msg("user", &i.to_string()))
            .collect();
        let result = NoopCompactor.compact(messages, 100);
        for (i, m) in result.iter().enumerate() {
            assert_eq!(m.content, i.to_string());
        }
    }
}
