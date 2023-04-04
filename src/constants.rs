use crate::types::Timestamp;

const MINUTES: Timestamp = 60;
pub(crate) const HOURS: Timestamp = 60 * MINUTES;
pub(crate) const DAYS: Timestamp = 24 * HOURS;
pub(crate) const WEEKS: Timestamp = 7 * DAYS;

/// Base amount of time before a reporter is able to submit a value again.
pub(crate) const REPORTING_LOCK: Timestamp = 12 * HOURS;
