//! Message list handling for the WhatsApp UI

use std::rc::Rc;
use std::sync::Arc;

use gpui::{Pixels, Size, px, size};

use crate::state::{ChatMessage, MediaType};
use crate::theme::layout;
use crate::utils::scale_media_dimensions;

/// Cached data for message list rendering to avoid recomputing on every frame.
#[derive(Clone)]
pub struct MessageListCache {
    /// Message count when cache was created (invalidation check)
    pub message_count: usize,
    /// Group flag the sizes were computed with (invalidation check)
    pub is_group: bool,
    /// Media size cap the sizes were computed with (invalidation check)
    pub max_media_size: f32,
    /// Pre-computed item sizes for virtual list
    pub item_sizes: Rc<Vec<Size<Pixels>>>,
    /// Pre-computed show_sender flags for message grouping
    pub show_sender_flags: Arc<[bool]>,
    /// Shared messages reference
    pub messages: Arc<[ChatMessage]>,
}

impl MessageListCache {
    /// Create a new message list cache from messages.
    /// `max_media_size` should come from ResponsiveLayout for correct sizing.
    pub fn new(messages: &[ChatMessage], is_group: bool, max_media_size: f32) -> Self {
        let messages_arc: Arc<[ChatMessage]> = Arc::from(messages);

        let show_sender_flags: Arc<[bool]> = Arc::from(
            messages
                .iter()
                .enumerate()
                .map(|(i, _)| should_show_sender(messages, i))
                .collect::<Vec<_>>(),
        );

        let item_sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            messages
                .iter()
                .enumerate()
                .map(|(i, msg)| {
                    size(
                        px(600.),
                        px(calculate_message_height(
                            msg,
                            show_sender_flags[i],
                            is_group,
                            max_media_size,
                        )),
                    )
                })
                .collect(),
        );

        Self {
            message_count: messages.len(),
            is_group,
            max_media_size,
            item_sizes,
            show_sender_flags,
            messages: messages_arc,
        }
    }
}

/// Check if this message should show the sender name (for grouping)
pub fn should_show_sender(messages: &[ChatMessage], index: usize) -> bool {
    if index == 0 {
        return true;
    }
    let current = &messages[index];
    let previous = &messages[index - 1];
    current.sender != previous.sender || current.is_from_me != previous.is_from_me
}

/// Calculate the height needed for a message bubble.
/// `show_sender` must be the same raw grouping flag the bubble renders with:
/// it drives the outer padding in every chat, while the sender-name line only
/// exists in groups. `max_media_size` should come from ResponsiveLayout.
pub fn calculate_message_height(
    msg: &ChatMessage,
    show_sender: bool,
    is_group: bool,
    max_media_size: f32,
) -> f32 {
    let outer_top = if show_sender {
        layout::MSG_PADDING_TOP_FIRST
    } else {
        layout::MSG_PADDING_TOP_GROUPED
    };
    let mut height = outer_top
        + layout::MSG_PADDING_BOTTOM
        + (layout::MSG_BUBBLE_PADDING_Y * 2.0)
        + layout::MSG_TIME_ROW_HEIGHT;

    let mut content_items = 1;

    if is_group && show_sender && msg.sender_name.is_some() && !msg.is_from_me {
        height += layout::MSG_SENDER_NAME_HEIGHT;
        content_items += 1;
    }

    if let Some(media) = &msg.media {
        let media_h = match media.media_type {
            MediaType::Image | MediaType::Sticker | MediaType::Video => {
                let (_, h) = scale_media_dimensions(
                    media.width.unwrap_or(300),
                    media.height.unwrap_or(300),
                    max_media_size,
                );
                h
            }
            MediaType::Audio => 44.0,
            MediaType::Document => 50.0,
        };
        height += media_h;
        content_items += 1;
    }

    if !msg.content.is_empty() {
        let char_count = msg.content.chars().count();
        let newlines = msg.content.matches('\n').count();
        let wrapped_lines = char_count.div_ceil(30);
        let lines = (wrapped_lines + newlines).max(1);
        height += lines as f32 * layout::MSG_TEXT_LINE_HEIGHT;
        content_items += 1;
    }

    if content_items > 1 {
        height += (content_items - 1) as f32 * layout::MSG_CONTENT_GAP;
    }

    if !msg.reactions.is_empty() {
        height += layout::MSG_REACTION_MARGIN_TOP + layout::MSG_REACTION_HEIGHT;
    }

    height
}
