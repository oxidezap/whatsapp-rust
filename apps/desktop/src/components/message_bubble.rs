//! Message bubble component with responsive layout support.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    Entity, Image, ImageSource, ObjectFit, RenderImage, SharedString, div, img, prelude::*, px, rgb,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::clipboard::Clipboard;
use gpui_component::h_flex;
use gpui_component::v_flex;
use gpui_component::{Disableable, Icon};

use crate::app::WhatsAppApp;
use crate::responsive::ResponsiveLayout;
use crate::state::{ChatMessage, DownloadableMedia, MediaType};
use crate::theme::{colors, layout};
use crate::utils::{format_time_local, mime_to_image_format, scale_media_dimensions};
use crate::video::VideoPlayerState;

pub fn render_message_bubble(
    message: ChatMessage,
    entity: Entity<WhatsAppApp>,
    playing_message_id: Option<String>,
    is_group: bool,
    show_sender: bool,
    video_player_state: Option<VideoPlayerState>,
    video_frame: Option<Arc<RenderImage>>,
    sticker_image: Option<Arc<Image>>,
    responsive_layout: ResponsiveLayout,
) -> impl IntoElement {
    let is_from_me = message.is_from_me;
    let message_id = message.id.clone();
    let content: SharedString = message.content.clone().into();
    let time: SharedString = format_time_local(&message.timestamp).into();
    let media = message.media.clone();
    let content_for_copy = message.content.clone();
    let bubble_id: SharedString = format!("msg-{}", message.id).into();
    let is_playing = playing_message_id.as_ref() == Some(&message_id);
    let send_failed = is_from_me && message.failed;
    let reactions = message.reactions.clone();
    let has_reactions = !reactions.is_empty();
    let sender_name: Option<SharedString> = if is_group && !is_from_me && show_sender {
        message.sender_name.clone().map(|s| s.into())
    } else {
        None
    };

    div()
        .w_full()
        .flex()
        .map(|el| {
            if is_from_me {
                el.justify_end()
            } else {
                el.justify_start()
            }
        })
        .pt(px(if show_sender {
            layout::MSG_PADDING_TOP_FIRST
        } else {
            layout::MSG_PADDING_TOP_GROUPED
        }))
        .pb(px(layout::MSG_PADDING_BOTTOM))
        .child(
            v_flex()
                .items_end()
                .when(!is_from_me, |el| el.items_start())
                .child(
                    div()
                        .id(bubble_id.clone())
                        .max_w(px(responsive_layout.max_bubble_width()))
                        .px(px(layout::MSG_BUBBLE_PADDING_X))
                        .py(px(layout::MSG_BUBBLE_PADDING_Y))
                        .rounded(px(layout::RADIUS_MEDIUM))
                        .bg(if is_from_me {
                            rgb(colors::BG_MESSAGE_SENT)
                        } else {
                            rgb(colors::BG_MESSAGE_RECEIVED)
                        })
                        .child(
                            v_flex()
                                .gap(px(layout::MSG_CONTENT_GAP))
                                .when_some(sender_name, |el, name| {
                                    el.child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(rgb(colors::ACCENT_GREEN))
                                            .child(name),
                                    )
                                })
                                .when_some(media, |el, media_content| {
                                    render_media_content(
                                        el,
                                        media_content,
                                        message_id.clone(),
                                        is_playing,
                                        entity.clone(),
                                        video_player_state,
                                        video_frame.clone(),
                                        sticker_image.clone(),
                                        responsive_layout.max_media_size(),
                                    )
                                })
                                .when(!content.is_empty(), |el| {
                                    el.child(
                                        div()
                                            .overflow_hidden()
                                            .text_color(rgb(colors::TEXT_PRIMARY))
                                            .child(content),
                                    )
                                })
                                // Time and copy button row
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_color(rgb(colors::TEXT_SECONDARY))
                                                .text_xs()
                                                .child(time),
                                        )
                                        .when(send_failed, |el| {
                                            el.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(colors::ERROR))
                                                    .child("failed"),
                                            )
                                        })
                                        .when(!content_for_copy.is_empty(), |el| {
                                            el.child(
                                                Clipboard::new(bubble_id).value(content_for_copy),
                                            )
                                        }),
                                ),
                        ),
                )
                .when(has_reactions, |el| {
                    el.child(render_reactions(reactions, is_from_me))
                }),
        )
}

fn render_reactions(reactions: HashMap<String, Vec<String>>, is_from_me: bool) -> impl IntoElement {
    let mut sorted_reactions: Vec<_> = reactions.into_iter().collect();
    sorted_reactions.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));

    h_flex()
        .gap_1()
        .mt(px(layout::MSG_REACTION_MARGIN_TOP))
        .h(px(layout::MSG_REACTION_HEIGHT))
        .map(|el| {
            if is_from_me {
                el.justify_end()
            } else {
                el.justify_start()
            }
        })
        .px_1()
        .children(sorted_reactions.into_iter().map(|(emoji, senders)| {
            let count = senders.len();
            let emoji_str: SharedString = emoji.into();

            div()
                .px(px(6.))
                .py(px(2.))
                .rounded(px(12.))
                .bg(rgb(colors::BG_SELECTED))
                .border_1()
                .border_color(rgb(colors::BORDER))
                .flex()
                .items_center()
                .gap(px(2.))
                .child(div().text_sm().child(emoji_str))
                .when(count > 1, |el| {
                    el.child(
                        div()
                            .text_xs()
                            .text_color(rgb(colors::TEXT_SECONDARY))
                            .child(count.to_string()),
                    )
                })
        }))
}

fn render_media_content(
    el: gpui::Div,
    media_content: crate::state::MediaContent,
    message_id: String,
    is_playing: bool,
    entity: Entity<WhatsAppApp>,
    video_player_state: Option<VideoPlayerState>,
    video_frame: Option<Arc<RenderImage>>,
    sticker_image: Option<Arc<Image>>,
    max_media_size: f32,
) -> gpui::Div {
    match media_content.media_type {
        MediaType::Image => {
            let (display_w, display_h) = scale_media_dimensions(
                media_content.width.unwrap_or(300),
                media_content.height.unwrap_or(300),
                max_media_size,
            );

            if !media_content.data.is_empty() {
                let image = render_image_from_bytes(
                    media_content.data,
                    &media_content.mime_type,
                    display_w,
                    display_h,
                    true,
                );
                if media_content.data_is_preview
                    && let Some(dl) = media_content.downloadable.clone()
                {
                    // Only the fallback thumbnail is local: tapping it fetches
                    // the full image, same path as the empty placeholder
                    let preview_id: SharedString = format!("img-preview-{}", message_id).into();
                    el.child(
                        div()
                            .id(preview_id)
                            .cursor_pointer()
                            .on_click(move |_, _window, cx| {
                                let msg_id = message_id.clone();
                                let dl = dl.clone();
                                entity.update(cx, |app, cx| {
                                    app.download_image(msg_id, dl, cx);
                                });
                            })
                            .child(image),
                    )
                } else {
                    el.child(image)
                }
            } else if let Some(dl) = media_content.downloadable.clone() {
                // Eager download failed but the metadata survived: keep the
                // image fetchable on tap, like audio/video already are.
                el.child(render_download_placeholder(
                    "img-dl",
                    "[Image] Tap to download",
                    message_id,
                    dl,
                    entity,
                    display_w,
                    display_h,
                ))
            } else {
                el.child(render_media_placeholder("[Image]", display_w, display_h))
            }
        }
        MediaType::Sticker => {
            let (display_w, display_h) = scale_media_dimensions(
                media_content.width.unwrap_or(300),
                media_content.height.unwrap_or(300),
                max_media_size,
            );

            if media_content.data_is_preview
                && !media_content.data.is_empty()
                && let Some(dl) = media_content.downloadable.clone()
            {
                // Only the fallback PNG thumbnail is local: tapping fetches
                // the real sticker, mirroring the image preview branch.
                let image = render_image_from_bytes(
                    media_content.data,
                    &media_content.mime_type,
                    display_w,
                    display_h,
                    false,
                );
                let preview_id: SharedString = format!("sticker-preview-{}", message_id).into();
                el.child(
                    div()
                        .id(preview_id)
                        .cursor_pointer()
                        .on_click(move |_, _window, cx| {
                            let msg_id = message_id.clone();
                            let dl = dl.clone();
                            entity.update(cx, |app, cx| {
                                app.download_image(msg_id, dl, cx);
                            });
                        })
                        .child(image),
                )
            } else if let Some(cached_image) = sticker_image {
                let sticker_id: SharedString = format!("sticker-{}", message_id).into();
                el.child(
                    img(ImageSource::Image(cached_image))
                        .id(sticker_id)
                        .w(px(display_w))
                        .h(px(display_h))
                        .object_fit(gpui::ObjectFit::Contain),
                )
            } else if !media_content.data.is_empty() {
                el.child(render_image_from_bytes(
                    media_content.data,
                    &media_content.mime_type,
                    display_w,
                    display_h,
                    false,
                ))
            } else if let Some(dl) = media_content.downloadable.clone() {
                // Hydrated stickers (and failed eager downloads without a
                // thumbnail) carry only metadata: fetch on tap like images.
                el.child(render_download_placeholder(
                    "sticker-dl",
                    "[Sticker] Tap to download",
                    message_id,
                    dl,
                    entity,
                    display_w,
                    display_h,
                ))
            } else {
                el.child(render_media_placeholder("[Sticker]", display_w, display_h))
            }
        }
        MediaType::Video => el.child(render_video_player(
            media_content,
            message_id,
            entity,
            video_player_state,
            video_frame,
            max_media_size,
        )),
        MediaType::Audio => el.child(render_audio_player(
            media_content,
            message_id,
            is_playing,
            entity,
        )),
        MediaType::Document => el.child(render_document_placeholder(
            media_content,
            message_id,
            entity,
        )),
    }
}

/// Tap-to-download placeholder for media whose bytes aren't local yet, sized
/// like the real media so virtual-list row heights don't jump on arrival.
fn render_download_placeholder(
    id_prefix: &str,
    label: &'static str,
    message_id: String,
    dl: DownloadableMedia,
    entity: Entity<WhatsAppApp>,
    width: f32,
    height: f32,
) -> impl IntoElement {
    let placeholder_id: SharedString = format!("{id_prefix}-{message_id}").into();
    div()
        .id(placeholder_id)
        .w(px(width))
        .h(px(height))
        .bg(rgb(colors::BG_SELECTED))
        .rounded(px(layout::RADIUS_SMALL))
        .cursor_pointer()
        .flex()
        .justify_center()
        .items_center()
        .child(div().text_color(rgb(colors::TEXT_SECONDARY)).child(label))
        .on_click(move |_, _window, cx| {
            let msg_id = message_id.clone();
            let dl = dl.clone();
            entity.update(cx, |app, cx| {
                app.download_image(msg_id, dl, cx);
            });
        })
}

fn render_media_placeholder(text: &'static str, width: f32, height: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .h(px(height))
        .bg(rgb(colors::BG_SELECTED))
        .rounded(px(layout::RADIUS_SMALL))
        .flex()
        .justify_center()
        .items_center()
        .child(div().text_color(rgb(colors::TEXT_SECONDARY)).child(text))
}

fn render_image_from_bytes(
    data: Arc<Vec<u8>>,
    mime_type: &str,
    width: f32,
    height: f32,
    rounded: bool,
) -> gpui::Img {
    let format = mime_to_image_format(mime_type);
    let image_data = Arc::unwrap_or_clone(data);
    let image = Image::from_bytes(format, image_data);

    let img_el = img(ImageSource::Image(Arc::new(image)))
        .w(px(width))
        .h(px(height))
        .object_fit(gpui::ObjectFit::Contain);

    if rounded {
        img_el.rounded(px(layout::RADIUS_SMALL))
    } else {
        img_el
    }
}

fn render_audio_player(
    media_content: crate::state::MediaContent,
    message_id: String,
    is_playing: bool,
    entity: Entity<WhatsAppApp>,
) -> impl IntoElement {
    let has_data = media_content.has_data();
    let can_download = media_content.can_download();
    let can_play = has_data || can_download;
    let downloadable = media_content.downloadable.clone();
    let button_id: SharedString = format!("play-{}", message_id).into();
    let duration_text: SharedString = if let Some(secs) = media_content.duration_secs {
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs).into()
    } else {
        "Voice message".into()
    };

    div()
        .w(px(220.))
        .h(px(44.))
        .bg(rgb(colors::BG_SELECTED))
        .rounded(px(layout::RADIUS_LARGE))
        .flex()
        .items_center()
        .px_2()
        .gap_2()
        .child(
            Button::new(button_id)
                .icon(
                    Icon::default()
                        .path(if is_playing {
                            "icons/pause.svg"
                        } else {
                            "icons/play.svg"
                        })
                        .text_color(rgb(colors::TEXT_PRIMARY)),
                )
                .ghost()
                .disabled(!can_play)
                .on_click({
                    let data = media_content.data.clone();
                    let downloadable = downloadable.clone();
                    move |_, _window, cx| {
                        let msg_id = message_id.clone();
                        entity.update(cx, |app, cx| {
                            if !data.is_empty() {
                                app.toggle_audio(msg_id, (*data).clone(), cx);
                            } else if let Some(dl) = downloadable.clone() {
                                app.toggle_audio_lazy(msg_id, dl, cx);
                            }
                        });
                    }
                }),
        )
        .child(
            div()
                .flex_1()
                .h(px(24.))
                .rounded(px(4.))
                .bg(rgb(if is_playing {
                    colors::ACCENT_GREEN
                } else {
                    colors::BG_HOVER
                }))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(colors::TEXT_SECONDARY))
                        .child(match (is_playing, !has_data && can_download) {
                            (true, _) => SharedString::from("Playing..."),
                            (_, true) => SharedString::from("Tap to download"),
                            _ => duration_text,
                        }),
                ),
        )
}

fn render_document_placeholder(
    media_content: crate::state::MediaContent,
    message_id: String,
    entity: Entity<WhatsAppApp>,
) -> impl IntoElement {
    let label: SharedString = media_content
        .file_name
        .clone()
        .unwrap_or_else(|| "Document".to_string())
        .into();
    let row = div()
        .w(px(200.))
        .h(px(50.))
        .bg(rgb(colors::BG_SELECTED))
        .rounded(px(layout::RADIUS_MEDIUM))
        .flex()
        .items_center()
        .px_3()
        .gap_2()
        .child(
            div()
                .overflow_hidden()
                .text_color(rgb(colors::TEXT_SECONDARY))
                .child(label),
        );

    // Doc bytes are never cached for rendering; with download metadata the
    // row saves the file to the Downloads dir on tap.
    if let Some(dl) = media_content.downloadable {
        let file_name = media_content
            .file_name
            .unwrap_or_else(|| "document".to_string());
        let doc_id: SharedString = format!("doc-{}", message_id).into();
        row.id(doc_id)
            .cursor_pointer()
            .on_click(move |_, _window, cx| {
                let msg_id = message_id.clone();
                let name = file_name.clone();
                let dl = dl.clone();
                entity.update(cx, |app, cx| {
                    app.download_document(msg_id, name, dl, cx);
                });
            })
            .into_any_element()
    } else {
        row.into_any_element()
    }
}

fn render_video_player(
    media_content: crate::state::MediaContent,
    message_id: String,
    entity: Entity<WhatsAppApp>,
    video_player_state: Option<VideoPlayerState>,
    video_frame: Option<Arc<RenderImage>>,
    max_media_size: f32,
) -> impl IntoElement {
    let (display_w, display_h) = scale_media_dimensions(
        media_content.width.unwrap_or(300),
        media_content.height.unwrap_or(200),
        max_media_size,
    );

    let button_id: SharedString = format!("video-{}", message_id).into();
    let state = video_player_state.unwrap_or(VideoPlayerState::Idle);
    let downloadable = media_content.downloadable.clone();
    let can_download = media_content.can_download();
    let is_playing = state.is_playing();
    let is_paused = state.is_paused();
    let is_loading = state.is_loading();
    let is_error = state.is_error();

    div()
        .relative()
        .w(px(display_w))
        .h(px(display_h))
        .rounded(px(layout::RADIUS_SMALL))
        .overflow_hidden()
        .child(
            if let Some(frame) = video_frame.filter(|_| is_playing || is_paused) {
                // Frame is a pre-decoded RGBA `RenderImage`; render with the
                // standard `img()` element. GPU-side YUV surfaces (the old
                // `surface()` path) are macOS-only upstream.
                div()
                    .w_full()
                    .h_full()
                    .child(
                        img(frame)
                            .w(px(display_w))
                            .h(px(display_h))
                            .object_fit(ObjectFit::Contain),
                    )
                    .into_any_element()
            } else if !media_content.data.is_empty() {
                div()
                    .w_full()
                    .h_full()
                    .child(render_image_from_bytes(
                        media_content.data,
                        &media_content.mime_type,
                        display_w,
                        display_h,
                        false,
                    ))
                    .into_any_element()
            } else {
                div()
                    .w_full()
                    .h_full()
                    .bg(rgb(colors::BG_SELECTED))
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(
                        div()
                            .text_color(rgb(colors::TEXT_SECONDARY))
                            .child("[Video]"),
                    )
                    .into_any_element()
            },
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .justify_center()
                .items_center()
                .bg(gpui::rgba(0x00000066))
                .when(is_playing, |el| el.bg(gpui::rgba(0x00000000)))
                .child(if is_loading {
                    div()
                        .w(px(48.))
                        .h(px(48.))
                        .rounded_full()
                        .bg(gpui::rgba(0x00000088))
                        .flex()
                        .justify_center()
                        .items_center()
                        .child(div().text_color(rgb(colors::TEXT_PRIMARY)).text_sm().child(
                            if state == VideoPlayerState::Downloading {
                                "↓"
                            } else {
                                "⏳"
                            },
                        ))
                        .into_any_element()
                } else if is_error {
                    div()
                        .w(px(48.))
                        .h(px(48.))
                        .rounded_full()
                        .bg(gpui::rgba(0xFF000088))
                        .flex()
                        .justify_center()
                        .items_center()
                        .child(
                            div()
                                .text_color(rgb(colors::TEXT_PRIMARY))
                                .text_sm()
                                .child("⚠"),
                        )
                        .into_any_element()
                } else if !is_playing {
                    Button::new(button_id)
                        .icon(
                            Icon::default()
                                .path("icons/play.svg")
                                .text_color(rgb(colors::TEXT_PRIMARY))
                                .size(px(32.)),
                        )
                        .ghost()
                        .disabled(!can_download)
                        .on_click({
                            let downloadable = downloadable.clone();
                            move |_, _window, cx| {
                                if let Some(dl) = downloadable.clone() {
                                    let msg_id = message_id.clone();
                                    entity.update(cx, |app, cx| {
                                        app.toggle_video(msg_id, dl, cx);
                                    });
                                }
                            }
                        })
                        .into_any_element()
                } else {
                    Button::new(button_id)
                        .icon(
                            Icon::default()
                                .path("icons/pause.svg")
                                .text_color(gpui::rgba(0xFFFFFF66))
                                .size(px(24.)),
                        )
                        .ghost()
                        .on_click({
                            let downloadable = downloadable.clone();
                            move |_, _window, cx| {
                                if let Some(dl) = downloadable.clone() {
                                    let msg_id = message_id.clone();
                                    entity.update(cx, |app, cx| {
                                        app.toggle_video(msg_id, dl, cx);
                                    });
                                }
                            }
                        })
                        .into_any_element()
                }),
        )
}
