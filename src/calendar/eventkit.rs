use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use objc2::rc::Retained;
use objc2_event_kit::{
    EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStore,
    EKEventStoreChangedNotification,
};
use objc2_foundation::{NSArray, NSDate, NSNotificationCenter};

use crate::db::queries::{Calendar, CalendarEvent};
use crate::meeting_url;

#[derive(Debug)]
pub enum CalendarError {
    AccessDenied,
    AccessRestricted,
    EventKitError(String),
}

impl std::fmt::Display for CalendarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalendarError::AccessDenied => write!(f, "Calendar access denied"),
            CalendarError::AccessRestricted => write!(f, "Calendar access restricted"),
            CalendarError::EventKitError(msg) => write!(f, "EventKit error: {msg}"),
        }
    }
}

impl std::error::Error for CalendarError {}

pub struct EventKitStore {
    store: Retained<EKEventStore>,
    changed: Arc<AtomicBool>,
    // Hold the observer to prevent it from being deallocated
    _observer: Option<Retained<objc2::runtime::ProtocolObject<dyn objc2::runtime::NSObjectProtocol>>>,
}

impl EventKitStore {
    pub fn new() -> Self {
        let store = unsafe { EKEventStore::new() };
        let changed = Arc::new(AtomicBool::new(false));

        // Subscribe to calendar change notifications
        let changed_flag = changed.clone();
        let block = block2::RcBlock::new(move |_notification: std::ptr::NonNull<objc2_foundation::NSNotification>| {
            changed_flag.store(true, Ordering::Relaxed);
            tracing::debug!("Calendar data changed (EKEventStoreChangedNotification)");
        });

        let observer = unsafe {
            let center = NSNotificationCenter::defaultCenter();
            center.addObserverForName_object_queue_usingBlock(
                Some(EKEventStoreChangedNotification),
                Some(&store),
                None, // deliver on posting thread
                &block,
            )
        };

        Self {
            store,
            changed,
            _observer: Some(observer),
        }
    }

    /// Returns true if the calendar data has changed since the last call.
    /// Resets the flag after reading.
    pub fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::Relaxed)
    }

    fn authorization_status() -> EKAuthorizationStatus {
        unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) }
    }

    pub fn request_access(&self) -> Result<bool, CalendarError> {
        let status = Self::authorization_status();
        if status == EKAuthorizationStatus::FullAccess {
            return Ok(true);
        }
        if status == EKAuthorizationStatus::Denied {
            return Err(CalendarError::AccessDenied);
        }
        if status == EKAuthorizationStatus::Restricted {
            return Err(CalendarError::AccessRestricted);
        }

        let (tx, rx) = mpsc::channel();
        let block = block2::RcBlock::new(move |granted: objc2::runtime::Bool, _error: *mut objc2_foundation::NSError| {
            let _ = tx.send(granted.as_bool());
        });
        unsafe {
            let ptr: *mut block2::Block<dyn Fn(objc2::runtime::Bool, *mut objc2_foundation::NSError)> =
                std::ptr::from_ref(&*block).cast_mut();
            self.store.requestFullAccessToEventsWithCompletion(ptr);
        }

        let granted = rx.recv().map_err(|e| CalendarError::EventKitError(e.to_string()))?;
        if granted {
            Ok(true)
        } else {
            Err(CalendarError::AccessDenied)
        }
    }

    /// List all event calendars available on the system.
    pub fn fetch_calendars(&self) -> Vec<Calendar> {
        let ek_calendars = unsafe {
            self.store.calendarsForEntityType(EKEntityType::Event)
        };

        let mut calendars = Vec::new();
        for ek_cal in ek_calendars.iter() {
            let id = unsafe { ek_cal.calendarIdentifier().to_string() };
            let title = unsafe { ek_cal.title().to_string() };
            let color = unsafe {
                ek_cal.CGColor().map(|_color| {
                    // TODO: proper CGColor -> hex conversion
                    "#FF9500".to_string()
                })
            };
            calendars.push(Calendar {
                id,
                title,
                color,
                enabled: true, // new calendars default to enabled
            });
        }
        calendars
    }

    /// Fetch events in a time range, optionally filtered to specific calendar IDs.
    /// If `calendar_ids` is empty, fetches from all calendars.
    pub fn fetch_events(&self, from_timestamp: i64, to_timestamp: i64, calendar_ids: &[String]) -> Result<Vec<CalendarEvent>, CalendarError> {
        let now_secs = chrono::Utc::now().timestamp();

        let start_date = NSDate::dateWithTimeIntervalSince1970(from_timestamp as f64);
        let end_date = NSDate::dateWithTimeIntervalSince1970(to_timestamp as f64);

        // Build calendar filter if specific calendars are selected
        let calendars_filter: Option<Retained<NSArray<EKCalendar>>> = if calendar_ids.is_empty() {
            None
        } else {
            let all_calendars = unsafe {
                self.store.calendarsForEntityType(EKEntityType::Event)
            };
            let filtered: Vec<Retained<EKCalendar>> = all_calendars
                .iter()
                .filter(|cal| {
                    let cal_id = unsafe { cal.calendarIdentifier().to_string() };
                    calendar_ids.contains(&cal_id)
                })
                .map(|cal| cal.clone())
                .collect();

            if filtered.is_empty() {
                return Ok(Vec::new());
            }
            Some(NSArray::from_retained_slice(&filtered))
        };

        let predicate = unsafe {
            self.store.predicateForEventsWithStartDate_endDate_calendars(
                &start_date,
                &end_date,
                calendars_filter.as_deref(),
            )
        };

        let ek_events = unsafe {
            self.store.eventsMatchingPredicate(&predicate)
        };

        let mut events = Vec::new();
        for ek_event in ek_events.iter() {
            if let Some(event) = self.convert_event(&ek_event, now_secs) {
                events.push(event);
            }
        }

        Ok(events)
    }

    fn convert_event(&self, ek_event: &EKEvent, now_secs: i64) -> Option<CalendarEvent> {
        unsafe {
            let id = ek_event.eventIdentifier()?.to_string();
            let title = ek_event.title().to_string();
            let start = ek_event.startDate();
            let end = ek_event.endDate();
            let start_time = start.timeIntervalSince1970() as i64;
            let end_time = end.timeIntervalSince1970() as i64;

            if ek_event.isAllDay() {
                return None;
            }

            let location = ek_event.location().map(|s| s.to_string());
            let notes = ek_event.notes().map(|s| s.to_string());

            let meeting_url = meeting_url::extract_from_event(
                location.as_deref(),
                notes.as_deref(),
            );

            let (calendar_id, calendar_title, calendar_color) = if let Some(cal) = ek_event.calendar() {
                let cal_id = cal.calendarIdentifier().to_string();
                let cal_title = cal.title().to_string();
                let cal_color = cal.CGColor().map(|_color| {
                    // TODO: proper CGColor -> hex conversion
                    "#FF9500".to_string()
                });
                (cal_id, Some(cal_title), cal_color)
            } else {
                ("unknown".to_string(), None, None)
            };

            Some(CalendarEvent {
                id,
                calendar_id,
                calendar_title,
                calendar_color,
                title,
                start_time,
                end_time,
                location,
                notes,
                meeting_url,
                last_synced: now_secs,
            })
        }
    }
}
