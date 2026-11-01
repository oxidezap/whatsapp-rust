//! Retained public contracts from historical whatspec captures. The new locked capture
//! omits these definitions; that does not establish server-side retirement.
//! Ringtone contracts came from whatspec 1a441f0; the video contracts cite c581c20 below.
use crate::{Channel, EventFields, WamEvent};

/// `RINGTONE_ENTRY_TYPE` (`WAWebWamEnumRingtoneEntryType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RingtoneEntryType {
    /// `APP_WIDE` = 0.
    AppWide,
    /// `ONE_TO_ONE` = 1.
    OneToOne,
    /// `GROUP` = 2.
    Group,
    /// `LIST` = 3.
    List,
}

impl RingtoneEntryType {
    /// The integer WA Web writes for this member.
    pub const fn wire(self) -> i64 {
        match self {
            Self::AppWide => 0,
            Self::OneToOne => 1,
            Self::Group => 2,
            Self::List => 3,
        }
    }
}

/// `RingtoneScreen`: WAM event 7608 on the `regular` channel.
///
/// Declared in `WAWebRingtoneScreenWamEvent`. Sampling weights `[1, 1, 1]` as the catalog lists them;
/// the weight a buffer carries can be overridden at runtime.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RingtoneScreen {
    /// `premiumRingtonesDownloadedCount` (id 1).
    pub premium_ringtones_downloaded_count: Option<i64>,
    /// `ringtoneChangeApplied` (id 2).
    pub ringtone_change_applied: Option<bool>,
    /// `ringtoneId` (id 3).
    pub ringtone_id: Option<String>,
    /// `ringtoneReset` (id 7).
    pub ringtone_reset: Option<bool>,
    /// `ringtoneSelectionCancelled` (id 4).
    pub ringtone_selection_cancelled: Option<bool>,
    /// `ringtoneSource` (id 5).
    pub ringtone_source: Option<RingtoneEntryType>,
    /// `ringtoneSubscribeSelected` (id 6).
    pub ringtone_subscribe_selected: Option<bool>,
}

impl WamEvent for RingtoneScreen {
    const NAME: &'static str = "RingtoneScreen";
    const CODE: u32 = 7608;
    const CHANNEL: Channel = Channel::Regular;
    const WEIGHTS: [u32; 3] = [1, 1, 1];
    const PRIVATE_STATS_ID: Option<i64> = None;

    fn encode(&self, fields: &mut EventFields<'_>) {
        fields.integer(1, self.premium_ringtones_downloaded_count);
        fields.boolean(2, self.ringtone_change_applied);
        fields.string(3, self.ringtone_id.as_deref());
        fields.boolean(7, self.ringtone_reset);
        fields.boolean(4, self.ringtone_selection_cancelled);
        fields.integer(5, self.ringtone_source.map(|v| v.wire()));
        fields.boolean(6, self.ringtone_subscribe_selected);
    }
}

// Video telemetry retained from whatspec c581c20a9d0605df49abf297f2e7cfc04ccf1ff7.
// These public types are not declarations of the current capture.
use crate::enums;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaybackOriginType {
    /// `CONVERSATION` = 1.
    Conversation,
    /// `STATUS` = 2.
    Status,
    /// `CHANNELS` = 3.
    Channels,
}

impl PlaybackOriginType {
    /// The integer WA Web writes for this member.
    pub const fn wire(self) -> i64 {
        match self {
            Self::Conversation => 1,
            Self::Status => 2,
            Self::Channels => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaybackStateType {
    /// `IDLE` = 1.
    Idle,
    /// `READY_PLAY` = 2.
    ReadyPlay,
    /// `READY_PAUSE` = 3.
    ReadyPause,
    /// `BUFFERING` = 4.
    Buffering,
    /// `OUTSIDE` = 5.
    Outside,
    /// `ENDED` = 6.
    Ended,
    /// `ERROR` = 7.
    Error,
}

impl PlaybackStateType {
    /// The integer WA Web writes for this member.
    pub const fn wire(self) -> i64 {
        match self {
            Self::Idle => 1,
            Self::ReadyPlay => 2,
            Self::ReadyPause => 3,
            Self::Buffering => 4,
            Self::Outside => 5,
            Self::Ended => 6,
            Self::Error => 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoPlayOrigin {
    /// `CONVERSATION` = 1.
    Conversation,
    /// `GALLERY_PICKER` = 2.
    GalleryPicker,
    /// `STARRED_MESSAGES` = 3.
    StarredMessages,
    /// `MEDIA_VIEW_PAGER` = 4.
    MediaViewPager,
    /// `OTHER_ORIGIN` = 5.
    OtherOrigin,
    /// `CHANNELS` = 6.
    Channels,
    /// `STATUS` = 7.
    Status,
}

impl VideoPlayOrigin {
    /// The integer WA Web writes for this member.
    pub const fn wire(self) -> i64 {
        match self {
            Self::Conversation => 1,
            Self::GalleryPicker => 2,
            Self::StarredMessages => 3,
            Self::MediaViewPager => 4,
            Self::OtherOrigin => 5,
            Self::Channels => 6,
            Self::Status => 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoPlayResult {
    /// `OK` = 1.
    Ok,
    /// `ERROR_PLAYER` = 2.
    ErrorPlayer,
    /// `ERROR_VIDEO_TRACK` = 3.
    ErrorVideoTrack,
    /// `ERROR_AUDIO_TRACK` = 4.
    ErrorAudioTrack,
    /// `ERROR_DOWNLOAD_FAILED` = 5.
    ErrorDownloadFailed,
}

impl VideoPlayResult {
    /// The integer WA Web writes for this member.
    pub const fn wire(self) -> i64 {
        match self {
            Self::Ok => 1,
            Self::ErrorPlayer => 2,
            Self::ErrorVideoTrack => 3,
            Self::ErrorAudioTrack => 4,
            Self::ErrorDownloadFailed => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoPlayType {
    /// `FILE` = 1.
    File,
    /// `STREAM` = 2.
    Stream,
    /// `SENT` = 3.
    Sent,
}

impl VideoPlayType {
    /// The integer WA Web writes for this member.
    pub const fn wire(self) -> i64 {
        match self {
            Self::File => 1,
            Self::Stream => 2,
            Self::Sent => 3,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChannelsVideoPlay {
    /// `autoPlayT` (id 1).
    pub auto_play_t: Option<i64>,
    /// `cid` (id 2).
    pub cid: Option<String>,
    /// `finishCount` (id 13).
    pub finish_count: Option<i64>,
    /// `height` (id 3).
    pub height: Option<i64>,
    /// `postId` (id 4).
    pub post_id: Option<String>,
    /// `videoDuration` (id 5).
    pub video_duration: Option<i64>,
    /// `videoInitialBufferingT` (id 6). A duration in milliseconds.
    pub video_initial_buffering_t: Option<i64>,
    /// `videoPlayOrigin` (id 7).
    pub video_play_origin: Option<VideoPlayOrigin>,
    /// `videoPlayResult` (id 8).
    pub video_play_result: Option<VideoPlayResult>,
    /// `videoPlayT` (id 9).
    pub video_play_t: Option<i64>,
    /// `videoPlayType` (id 10).
    pub video_play_type: Option<VideoPlayType>,
    /// `videoSize` (id 11).
    pub video_size: Option<f64>,
    /// `watchingModule` (id 14).
    pub watching_module: Option<String>,
    /// `width` (id 12).
    pub width: Option<i64>,
}

impl WamEvent for ChannelsVideoPlay {
    const NAME: &'static str = "ChannelsVideoPlay";
    const CODE: u32 = 6556;
    const CHANNEL: Channel = Channel::Regular;
    const WEIGHTS: [u32; 3] = [1, 1, 1];
    const PRIVATE_STATS_ID: Option<i64> = None;

    fn encode(&self, fields: &mut EventFields<'_>) {
        fields.integer(1, self.auto_play_t);
        fields.string(2, self.cid.as_deref());
        fields.integer(13, self.finish_count);
        fields.integer(3, self.height);
        fields.string(4, self.post_id.as_deref());
        fields.integer(5, self.video_duration);
        fields.integer(6, self.video_initial_buffering_t);
        fields.integer(7, self.video_play_origin.map(|v| v.wire()));
        fields.integer(8, self.video_play_result.map(|v| v.wire()));
        fields.integer(9, self.video_play_t);
        fields.integer(10, self.video_play_type.map(|v| v.wire()));
        fields.number(11, self.video_size);
        fields.string(14, self.watching_module.as_deref());
        fields.integer(12, self.width);
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaStreamPlayback {
    /// `bytesDownloadedStart` (id 4).
    pub bytes_downloaded_start: Option<f64>,
    /// `bytesTransferred` (id 5).
    pub bytes_transferred: Option<f64>,
    /// `didPlay` (id 15).
    pub did_play: Option<bool>,
    /// `experienceIds` (id 19).
    pub experience_ids: Option<String>,
    /// `forcedPlayCount` (id 12).
    pub forced_play_count: Option<i64>,
    /// `initialBufferingT` (id 7). A duration in milliseconds.
    pub initial_buffering_t: Option<i64>,
    /// `mediaSize` (id 2).
    pub media_size: Option<f64>,
    /// `mediaType` (id 3).
    pub media_type: Option<enums::MediaType>,
    /// `overallPlayT` (id 10). A duration in milliseconds.
    pub overall_play_t: Option<i64>,
    /// `overallT` (id 1). A duration in milliseconds.
    pub overall_t: Option<i64>,
    /// `playbackCount` (id 14).
    pub playback_count: Option<i64>,
    /// `playbackError` (id 17).
    pub playback_error: Option<i64>,
    /// `playbackOrigin` (id 16).
    pub playback_origin: Option<PlaybackOriginType>,
    /// `playbackState` (id 11).
    pub playback_state: Option<PlaybackStateType>,
    /// `seekCount` (id 13).
    pub seek_count: Option<i64>,
    /// `statusId` (id 18).
    pub status_id: Option<String>,
    /// `totalRebufferingCount` (id 9).
    pub total_rebuffering_count: Option<i64>,
    /// `totalRebufferingT` (id 8). A duration in milliseconds.
    pub total_rebuffering_t: Option<i64>,
    /// `videoDuration` (id 6).
    pub video_duration: Option<i64>,
}

impl WamEvent for MediaStreamPlayback {
    const NAME: &'static str = "MediaStreamPlayback";
    const CODE: u32 = 1584;
    const CHANNEL: Channel = Channel::Regular;
    const WEIGHTS: [u32; 3] = [1, 1, 1];
    const PRIVATE_STATS_ID: Option<i64> = None;

    fn encode(&self, fields: &mut EventFields<'_>) {
        fields.number(4, self.bytes_downloaded_start);
        fields.number(5, self.bytes_transferred);
        fields.boolean(15, self.did_play);
        fields.string(19, self.experience_ids.as_deref());
        fields.integer(12, self.forced_play_count);
        fields.integer(7, self.initial_buffering_t);
        fields.number(2, self.media_size);
        fields.integer(3, self.media_type.map(|v| v.wire()));
        fields.integer(10, self.overall_play_t);
        fields.integer(1, self.overall_t);
        fields.integer(14, self.playback_count);
        fields.integer(17, self.playback_error);
        fields.integer(16, self.playback_origin.map(|v| v.wire()));
        fields.integer(11, self.playback_state.map(|v| v.wire()));
        fields.integer(13, self.seek_count);
        fields.string(18, self.status_id.as_deref());
        fields.integer(9, self.total_rebuffering_count);
        fields.integer(8, self.total_rebuffering_t);
        fields.integer(6, self.video_duration);
    }
}
