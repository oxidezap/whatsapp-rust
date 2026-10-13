//! Retained public contracts from whatspec 1a441f0. The new locked capture
//! omits these definitions; that does not establish server-side retirement.
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
