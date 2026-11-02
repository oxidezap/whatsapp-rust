/// A durable host record survives reconnects; ordinary queued work does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryRecovery {
    Session,
    Durable,
}

impl HistoryRecovery {
    #[inline]
    pub fn stopped(
        self,
        shutting_down: impl FnOnce() -> bool,
        permanently_stopped: impl FnOnce() -> bool,
    ) -> bool {
        match self {
            Self::Session => shutting_down(),
            Self::Durable => permanently_stopped(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryAdmission {
    Process,
    RejectAndAcknowledge,
}

impl HistoryAdmission {
    /// Skip takes precedence and must not invoke the consumer policy.
    #[inline]
    pub fn decide(skip: bool, rejects: impl FnOnce() -> bool) -> Self {
        if skip || rejects() {
            Self::RejectAndAcknowledge
        } else {
            Self::Process
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryReceiptPoint {
    BeforePayload,
    Captured,
    Parsed,
}

/// Hook success is an adapter fact, not proof of storage durability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryPublication {
    recovery: HistoryRecovery,
    receipt: HistoryReceiptPoint,
}

impl HistoryPublication {
    #[inline]
    pub fn new(recovery: HistoryRecovery, captures_chunk: bool) -> Self {
        let receipt = match (recovery, captures_chunk) {
            (HistoryRecovery::Durable, _) => HistoryReceiptPoint::Parsed,
            (HistoryRecovery::Session, true) => HistoryReceiptPoint::Captured,
            (HistoryRecovery::Session, false) => HistoryReceiptPoint::BeforePayload,
        };
        Self { recovery, receipt }
    }

    #[inline]
    pub fn receipt_at(self, point: HistoryReceiptPoint) -> bool {
        self.receipt == point
    }

    #[inline]
    pub fn stopped(
        self,
        shutting_down: impl FnOnce() -> bool,
        permanently_stopped: impl FnOnce() -> bool,
    ) -> bool {
        self.recovery.stopped(shutting_down, permanently_stopped)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HistoryGeneration(usize);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HistoryActivitySnapshot {
    pub tasks: usize,
    pub tasks_peak: usize,
    pub payload_bytes: usize,
    pub payload_bytes_peak: usize,
}

#[derive(Debug, Default)]
pub struct HistoryActivity {
    generation: HistoryGeneration,
    snapshot: HistoryActivitySnapshot,
}

impl HistoryActivity {
    #[inline]
    pub fn begin(&mut self, payload_bytes: usize) -> HistoryGeneration {
        self.snapshot.tasks = self.snapshot.tasks.saturating_add(1);
        self.snapshot.tasks_peak = self.snapshot.tasks_peak.max(self.snapshot.tasks);
        self.snapshot.payload_bytes = self.snapshot.payload_bytes.saturating_add(payload_bytes);
        self.snapshot.payload_bytes_peak = self
            .snapshot
            .payload_bytes_peak
            .max(self.snapshot.payload_bytes);
        self.generation
    }

    #[inline]
    pub fn reset(&mut self) {
        self.generation.0 = self.generation.0.wrapping_add(1);
        self.snapshot.tasks = 0;
        self.snapshot.payload_bytes = 0;
    }

    #[inline]
    pub fn snapshot(&self) -> HistoryActivitySnapshot {
        self.snapshot
    }

    #[inline]
    pub fn resize(&mut self, generation: HistoryGeneration, previous: usize, current: usize) {
        if generation == self.generation {
            self.snapshot.payload_bytes = self
                .snapshot
                .payload_bytes
                .saturating_sub(previous)
                .saturating_add(current);
            self.snapshot.payload_bytes_peak = self
                .snapshot
                .payload_bytes_peak
                .max(self.snapshot.payload_bytes);
        }
    }

    /// Only the final owner of a task may release it. Stale owners never wake current waiters.
    #[inline]
    pub fn release(&mut self, generation: HistoryGeneration, payload_bytes: usize) -> bool {
        if generation != self.generation {
            return false;
        }
        self.snapshot.payload_bytes = self.snapshot.payload_bytes.saturating_sub(payload_bytes);
        self.snapshot.tasks = self.snapshot.tasks.saturating_sub(1);
        self.snapshot.tasks == 0
    }
}
