// Copyright 2023 Tellor Inc.
// This file is part of Tellor.

// Tellor is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Tellor is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Tellor. If not, see <http://www.gnu.org/licenses/>.

use crate::types::Timestamp;

pub const MINUTES: Timestamp = 60;
pub const HOURS: Timestamp = 60 * MINUTES;
pub const DAYS: Timestamp = 24 * HOURS;
pub const WEEKS: Timestamp = 7 * DAYS;

/// Base amount of time before a reporter is able to submit a value again.
pub(crate) const REPORTING_LOCK: Timestamp = 12 * HOURS;

/// The number of decimals of the TRB token.
pub(crate) const DECIMALS: u32 = 18;
