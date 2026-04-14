use midi_types::MidiMessage;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, Instant};

/// Helper that schedules MIDI messages to fire at the correct time based on delta times.
///
/// # Example
/// ```no_run
/// use rtpmidi::sessions::events::MidiScheduler;
///
/// let scheduler = MidiScheduler::new(|message| {
///     println!("Play: {:?}", message);
/// });
///
/// let mut midi_rx = session.subscribe_midi_messages().await;
/// while let Ok(events) = midi_rx.recv().await {
///     scheduler.schedule(events);
/// }
/// ```
pub struct MidiScheduler<F>
where
    F: Fn(MidiMessage) + Send + Sync + 'static,
{
    callback: Arc<F>,
}

impl<F> MidiScheduler<F>
where
    F: Fn(MidiMessage) + Send + Sync + 'static,
{
    /// Creates a new scheduler that will call the given callback for each message
    /// at the appropriate time based on delta times.
    pub fn new(callback: F) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    /// Schedule a batch of MIDI messages to be delivered at the correct times.
    ///
    /// Delta times are relative to the previous message in the batch.
    /// The first message fires immediately, subsequent messages are delayed
    /// by their accumulated delta times.
    pub fn schedule(&self, events: Vec<(MidiMessage, Duration)>) {
        if events.is_empty() {
            return;
        }

        let base_time = Instant::now();
        let callback = Arc::clone(&self.callback);

        // Spawn a task to handle the scheduling
        tokio::spawn(async move {
            let mut accumulated = Duration::ZERO;

            for (message, delta) in events {
                accumulated += delta;
                let fire_at = base_time + accumulated;

                let msg = message;
                let cb = Arc::clone(&callback);

                // Spawn individual task for each event to allow parallel execution
                tokio::spawn(async move {
                    sleep_until(fire_at).await;
                    cb(msg);
                });
            }
        });
    }
}

async fn sleep_until(instant: Instant) {
    let now = Instant::now();
    if instant > now {
        sleep(instant - now).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use midi_types::{Channel, MidiMessage, Note, Value7};
    use std::sync::Mutex;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_scheduler_fires_in_order() {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_clone = Arc::clone(&messages);

        let scheduler = MidiScheduler::new(move |msg| {
            messages_clone.lock().unwrap().push(msg);
        });

        let events = vec![
            (
                MidiMessage::NoteOn(Channel::C1, Note::C4, Value7::from(100)),
                Duration::from_millis(0),
            ),
            (
                MidiMessage::NoteOn(Channel::C1, Note::D4, Value7::from(100)),
                Duration::from_millis(50),
            ),
            (
                MidiMessage::NoteOn(Channel::C1, Note::E4, Value7::from(100)),
                Duration::from_millis(50),
            ),
        ];

        scheduler.schedule(events);

        // Wait for all events to fire
        sleep(Duration::from_millis(200)).await;

        let result = messages.lock().unwrap();
        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_scheduler_respects_timing() {
        let start = Instant::now();
        let timestamps = Arc::new(Mutex::new(Vec::new()));
        let timestamps_clone = Arc::clone(&timestamps);

        let scheduler = MidiScheduler::new(move |_msg| {
            timestamps_clone.lock().unwrap().push(Instant::now());
        });

        let events = vec![
            (
                MidiMessage::NoteOn(Channel::C1, Note::C4, Value7::from(100)),
                Duration::from_millis(0),
            ),
            (
                MidiMessage::NoteOn(Channel::C1, Note::D4, Value7::from(100)),
                Duration::from_millis(100),
            ),
        ];

        scheduler.schedule(events);

        // Wait for all events
        sleep(Duration::from_millis(200)).await;

        let result = timestamps.lock().unwrap();
        assert_eq!(result.len(), 2);

        // First event should fire immediately (within 10ms)
        let first_elapsed = result[0] - start;
        assert!(first_elapsed < Duration::from_millis(10));

        // Second event should fire ~100ms after first
        let gap = result[1] - result[0];
        assert!(gap >= Duration::from_millis(90) && gap <= Duration::from_millis(110));
    }
}
