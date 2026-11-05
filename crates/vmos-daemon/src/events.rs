use std::collections::VecDeque;
use std::sync::Mutex;

use vmos_core::{Event, EventKind, Timestamp};

const DEFAULT_CAPACITY: usize = 1024;
const MAX_POLL: usize = 1000;

struct Inner {
    seq: u64,
    events: VecDeque<Event>,
}

pub struct EventBus {
    inner: Mutex<Inner>,
    capacity: usize,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                seq: 0,
                events: VecDeque::new(),
            }),
            capacity: capacity.max(1),
        }
    }

    pub fn publish(&self, vm: Option<&str>, kind: EventKind) -> Event {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        inner.seq += 1;
        let event = Event {
            seq: inner.seq,
            at: Timestamp::now(),
            vm: vm.map(str::to_string),
            kind,
        };
        inner.events.push_back(event.clone());
        while inner.events.len() > self.capacity {
            inner.events.pop_front();
        }
        event
    }

    /// Return events after `after` (exclusive) and the newest sequence number.
    pub fn poll(&self, after: Option<u64>, limit: usize) -> (Vec<Event>, u64) {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let after = after.unwrap_or(0);
        let limit = limit.clamp(1, MAX_POLL);
        let events: Vec<Event> = inner
            .events
            .iter()
            .filter(|event| event.seq > after)
            .take(limit)
            .cloned()
            .collect();
        (events, inner.seq)
    }

    pub fn latest(&self) -> u64 {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_bounds_history() {
        let bus = EventBus::new(4);
        for _ in 0..10 {
            bus.publish(Some("vm"), EventKind::VmStarted);
        }
        let (events, next) = bus.poll(None, 100);
        assert_eq!(events.len(), 4);
        assert_eq!(next, 10);
        assert_eq!(events.last().unwrap().seq, 10);
    }

    #[test]
    fn poll_filters_and_limits() {
        let bus = EventBus::new(10);
        bus.publish(None, EventKind::DaemonStarted);
        bus.publish(Some("a"), EventKind::VmCreated);
        bus.publish(Some("b"), EventKind::VmCreated);
        let (events, next) = bus.poll(Some(1), 1);
        assert_eq!(next, 3);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].seq, 2);
    }
}
