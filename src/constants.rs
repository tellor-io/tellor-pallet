use crate::types::Timestamp;

const MINUTE_IN_SECONDS: Timestamp = 60;
pub(crate) const HOUR_IN_SECONDS: Timestamp = 60 * MINUTE_IN_SECONDS;
pub(crate) const DAY_IN_SECONDS: Timestamp = 24 * HOUR_IN_SECONDS;
pub(crate) const WEEK_IN_SECONDS: Timestamp = 7 * DAY_IN_SECONDS;

/// The claim buffer time.
pub(crate) const CLAIM_BUFFER: Timestamp = 12 * HOUR_IN_SECONDS;

/// The claim period.
pub(crate) const CLAIM_PERIOD: Timestamp = 4 * WEEK_IN_SECONDS;

/// The dispute period.
pub(crate) const DISPUTE_PERIOD: Timestamp = 1 * DAY_IN_SECONDS;

/// Base amount of time before a reporter is able to submit a value again.
pub(crate) const REPORTING_LOCK: Timestamp = 12 * HOUR_IN_SECONDS;

/// The dispute period after a vote has been tallied.
pub(crate) const TALLIED_VOTE_DISPUTE_PERIOD: Timestamp = 1 * DAY_IN_SECONDS;

/// The withdrawal period.
pub(crate) const WITHDRAWAL_PERIOD: Timestamp = 7 * DAY_IN_SECONDS;
