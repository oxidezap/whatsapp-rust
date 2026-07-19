use wacore::appstate::patch_decode::WAPatchName;
use wacore::messages::DetachedHistorySyncNotification;

#[derive(Debug)]
pub enum MajorSyncTask {
    HistorySync {
        message_id: String,
        notification: Box<DetachedHistorySyncNotification>,
    },
    AppStateSync {
        name: WAPatchName,
        full_sync: bool,
    },
}
