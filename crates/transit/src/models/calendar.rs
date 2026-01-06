//! Service calendar for determining when trips run.
//!
//! Implements GTFS calendar.txt and calendar_dates.txt logic.

use chrono::{Datelike, NaiveDate, Weekday};
use std::collections::HashSet;
use std::sync::Arc;

use crate::identifiers::ServiceIdentifier;

/// Determines which days a transit service operates
#[derive(Clone, Debug)]
pub struct ServiceCalendar {
    pub service_id: ServiceIdentifier,

    // Regular schedule
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub weekdays: WeekdayFlags,

    // Exception dates
    pub added_dates: Arc<HashSet<NaiveDate>>,   // Service runs on these dates
    pub removed_dates: Arc<HashSet<NaiveDate>>, // Service does not run on these dates
}

/// Compact representation of which weekdays a service runs
#[derive(Clone, Copy, Debug, Default)]
pub struct WeekdayFlags {
    pub(crate) flags: u8,
}

impl WeekdayFlags {
    pub fn new() -> Self {
        Self { flags: 0 }
    }

    pub fn set(&mut self, weekday: Weekday) {
        self.flags |= 1 << weekday.number_from_monday();
    }

    pub fn unset(&mut self, weekday: Weekday) {
        self.flags &= !(1 << weekday.number_from_monday());
    }

    pub fn contains(&self, weekday: Weekday) -> bool {
        (self.flags & (1 << weekday.number_from_monday())) != 0
    }

    pub fn from_bools(mon: bool, tue: bool, wed: bool, thu: bool, fri: bool, sat: bool, sun: bool) -> Self {
        let mut flags = Self::new();
        if mon { flags.set(Weekday::Mon); }
        if tue { flags.set(Weekday::Tue); }
        if wed { flags.set(Weekday::Wed); }
        if thu { flags.set(Weekday::Thu); }
        if fri { flags.set(Weekday::Fri); }
        if sat { flags.set(Weekday::Sat); }
        if sun { flags.set(Weekday::Sun); }
        flags
    }
}

impl ServiceCalendar {
    /// Check if the service runs on a given date
    pub fn runs_on(&self, date: NaiveDate) -> bool {
        // Check explicit additions first
        if self.added_dates.contains(&date) {
            return true;
        }

        // Check explicit removals
        if self.removed_dates.contains(&date) {
            return false;
        }

        // Check regular schedule
        if date < self.start_date || date > self.end_date {
            return false;
        }

        self.weekdays.contains(date.weekday())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_weekday_flags() {
        let mut flags = WeekdayFlags::new();
        flags.set(Weekday::Mon);
        flags.set(Weekday::Wed);
        flags.set(Weekday::Fri);

        assert!(flags.contains(Weekday::Mon));
        assert!(!flags.contains(Weekday::Tue));
        assert!(flags.contains(Weekday::Wed));
    }

    #[test]
    fn test_service_calendar() {
        let calendar = ServiceCalendar {
            service_id: ServiceIdentifier::new("weekday"),
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            weekdays: WeekdayFlags::from_bools(true, true, true, true, true, false, false),
            added_dates: Arc::new(HashSet::from([
                NaiveDate::from_ymd_opt(2024, 7, 4).unwrap(), // Add July 4th (Thursday)
            ])),
            removed_dates: Arc::new(HashSet::from([
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(), // Remove New Year's Day (Monday)
            ])),
        };

        // Regular weekday
        assert!(calendar.runs_on(NaiveDate::from_ymd_opt(2024, 1, 2).unwrap())); // Tuesday

        // Weekend
        assert!(!calendar.runs_on(NaiveDate::from_ymd_opt(2024, 1, 6).unwrap())); // Saturday

        // Removed date
        assert!(!calendar.runs_on(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())); // Monday but removed

        // Added date
        assert!(calendar.runs_on(NaiveDate::from_ymd_opt(2024, 7, 4).unwrap())); // Thursday and added

        // Out of range
        assert!(!calendar.runs_on(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()));
    }
}
