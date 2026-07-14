//! Chat list item component with responsive dimensions.

use gpui::{Entity, SharedString, div, prelude::*, px, rgb};

use crate::app::WhatsAppApp;
use crate::responsive::ResponsiveLayout;
use crate::state::Chat;
use crate::theme::colors;

use super::Avatar;

fn single_line(text: String) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn render_chat_item(
    chat: Chat,
    is_selected: bool,
    jid: String,
    entity: Entity<WhatsAppApp>,
    layout: ResponsiveLayout,
) -> impl IntoElement {
    let name: SharedString = single_line(chat.name).into();
    let last_message: SharedString = chat
        .last_message
        .map(single_line)
        .unwrap_or_else(|| "No messages".to_string())
        .into();
    let unread = chat.unread_count;
    let manually_unread = chat.manually_unread;
    let initial = name.chars().next().unwrap_or('?');

    let bg = if is_selected {
        rgb(colors::BG_SELECTED)
    } else {
        rgb(colors::BG_SECONDARY)
    };

    div()
        .id(SharedString::from(format!("chat-{}", jid)))
        .w_full()
        .h(px(layout.chat_item_height()))
        .flex()
        .items_center()
        .px(px(layout.padding_small()))
        .gap(px(layout.gap()))
        .cursor_pointer()
        .bg(bg)
        .when(!is_selected, |el| el.hover(|s| s.bg(rgb(colors::BG_HOVER))))
        .on_click(move |_, window, cx| {
            entity.update(cx, |this, cx| this.select_chat(jid.clone(), window, cx));
        })
        .child(Avatar::from_initial(initial, layout.avatar_size()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .overflow_hidden()
                .child(
                    div().flex().min_w_0().justify_between().child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_color(rgb(colors::TEXT_PRIMARY))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(name),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .justify_between()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .text_color(rgb(colors::TEXT_SECONDARY))
                                .text_sm()
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .flex_1()
                                .child(last_message),
                        )
                        .when(unread > 0 || manually_unread, |el| {
                            // Manual mark-unread has no count: render a dot.
                            let label = if unread > 0 {
                                unread.to_string()
                            } else {
                                "\u{2022}".to_string()
                            };
                            el.child(
                                div()
                                    .flex_shrink_0()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgb(colors::ACCENT_GREEN))
                                    .text_color(rgb(colors::WHITE))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(label),
                            )
                        }),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::single_line;

    #[test]
    fn preview_whitespace_cannot_escape_the_row() {
        assert_eq!(
            single_line("Line one\nLine two\tLine three".to_string()),
            "Line one Line two Line three"
        );
    }
}
