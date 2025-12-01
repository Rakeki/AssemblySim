/// This module handles all time-related operations for the simulation
/// 
/// Key concepts:
/// - SimulationTime: A simple counter (measured in minutes or seconds)
/// - Event: Something that happens at a specific time
/// - EventQueue: Priority queue that processes events in time order

use std::collections::BinaryHeap;
use std::cmp::Ordering;

/// Represents a point in time during the simulation
/// We use u32 to keep it simple. You could measure this as:
/// - Number of seconds since simulation started
/// - Number of minutes since simulation started
/// For assembly lines, minutes makes more sense
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimulationTime(pub u32);

impl SimulationTime {
    /// Create a new time point
    pub fn new(minutes: u32) -> Self {
        SimulationTime(minutes)
    }

    /// Get the raw time value
    pub fn as_minutes(&self) -> u32 {
        self.0
    }

    /// Calculate duration between two times
    /// Example: time_later - time_earlier = duration
    pub fn duration_until(&self, other: SimulationTime) -> u32 {
        if other.0 >= self.0 {
            other.0 - self.0
        } else {
            0
        }
    }

    /// Add time to this time point
    /// Example: time_now + 30 minutes = process_end_time
    pub fn add_minutes(&self, minutes: u32) -> SimulationTime {
        SimulationTime(self.0 + minutes)
    }
}

/// Why we use u32 for cost:
/// u32 can represent 0 to 4,294,967,295
/// If measuring minutes: ~8,170 years of simulation
/// That's plenty for most assembly line simulations!

/// Represents what type of event happened
/// This helps us know what to do when an event occurs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    /// A process started on a machine
    ProcessStart { 
        machine_id: u32, 
        process_id: u32 
    },
    /// A process finished on a machine
    ProcessComplete { 
        machine_id: u32, 
        process_id: u32 
    },
    /// New material arrived at the production line
    MaterialArrival { 
        material_id: u32 
    },
    /// A staff member became available
    StaffAvailable { 
        staff_id: u32 
    },
    /// Staff was assigned to work on a machine
    StaffAssigned {
        staff_id: u32,
        machine_id: u32,
        process_id: u32,
    },
    /// Staff finished working on a machine (now available)
    StaffReleased {
        staff_id: u32,
        machine_id: u32,
    },
    /// A machine couldn't start because staff wasn't available
    StaffUnavailable {
        machine_id: u32,
        process_id: u32,
    },
}

/// An event that happens at a specific time
/// 
/// Example in real life:
/// - Time: 09:30 AM
/// - Event: "Machine A finished processing item #5"
/// 
/// In our simulation:
/// - time: SimulationTime(570) [9*60 + 30 = 570 minutes from start]
/// - event_type: ProcessComplete { machine_id: 0, process_id: 5 }
#[derive(Debug, Clone)]
pub struct Event {
    /// WHEN this event happens
    pub time: SimulationTime,
    /// WHAT type of event this is
    pub event_type: EventType,
}

/// We need these trait implementations so we can put Events in a BinaryHeap
/// BinaryHeap requires items to be orderable (have a priority)
impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for Event {}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// This is the KEY comparison function!
/// We reverse the normal order (other.cmp(self) instead of self.cmp(other))
/// so that BinaryHeap becomes a MIN-HEAP
/// 
/// MIN-HEAP = events with earliest times pop first
/// This is important for event-driven simulation!
impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse comparison makes it a min-heap
        other.time.cmp(&self.time)
    }
}

/// The core of our time simulation
/// 
/// Think of this as a calendar system:
/// - current_time: what time is it now in the simulation?
/// - event_queue: what events are scheduled in the future?
pub struct Simulator {
    /// The current simulation time
    pub current_time: SimulationTime,
    /// All future events, ordered by time
    /// BinaryHeap automatically keeps earliest events at the top
    event_queue: BinaryHeap<Event>,
}

impl Simulator {
    /// Create a new simulator starting at time 0
    pub fn new() -> Self {
        Simulator {
            current_time: SimulationTime::new(0),
            event_queue: BinaryHeap::new(),
        }
    }

    /// Schedule an event to happen at a specific time
    /// 
    /// Example:
    /// ```ignore
    /// let mut sim = Simulator::new();
    /// // Schedule a process to complete at time 30 (30 minutes from start)
    /// sim.schedule_event(
    ///     SimulationTime::new(30),
    ///     EventType::ProcessComplete { machine_id: 0, process_id: 1 }
    /// );
    /// ```
    pub fn schedule_event(&mut self, time: SimulationTime, event_type: EventType) {
        let event = Event { time, event_type };
        self.event_queue.push(event);
    }

    /// Check if there are more events to process
    pub fn has_events(&self) -> bool {
        !self.event_queue.is_empty()
    }

    /// Get the next event WITHOUT removing it from the queue
    /// This lets you peek at what's coming next
    pub fn peek_next_event(&self) -> Option<&Event> {
        self.event_queue.peek()
    }

    /// Get and remove the next event
    /// This is what you call inside your simulation loop
    pub fn next_event(&mut self) -> Option<Event> {
        self.event_queue.pop()
    }

    /// Process one event:
    /// 1. Pop the next event from the queue
    /// 2. Move current_time forward to when that event happens
    /// 3. Return the event so the caller can handle it
    /// 
    /// This is the main loop of your simulation!
    pub fn step(&mut self) -> Option<Event> {
        if let Some(event) = self.next_event() {
            // Move time forward to when this event happens
            self.current_time = event.time;
            Some(event)
        } else {
            None
        }
    }

    /// Run all events until the queue is empty
    /// This is useful for debugging or testing
    /// 
    /// The callback receives a mutable reference to the simulator and the event
    /// It can modify the simulator state or collect data about events
    pub fn run_all(&mut self, mut callback: impl FnMut(&mut Self, Event)) {
        while let Some(event) = self.step() {
            callback(self, event);
        }
    }

    /// Get how much time has passed since simulation start
    pub fn elapsed_time(&self) -> u32 {
        self.current_time.as_minutes()
    }

    /// Jump directly to a time (without processing events)
    pub fn set_time(&mut self, time: SimulationTime) {
        self.current_time = time;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulation_time_creation() {
        let time = SimulationTime::new(100);
        assert_eq!(time.as_minutes(), 100);
    }

    #[test]
    fn test_time_arithmetic() {
        let time1 = SimulationTime::new(10);
        let time2 = time1.add_minutes(20);
        assert_eq!(time2.as_minutes(), 30);
        assert_eq!(time1.duration_until(time2), 20);
    }

    #[test]
    fn test_simulator_creation() {
        let sim = Simulator::new();
        assert_eq!(sim.elapsed_time(), 0);
        assert!(!sim.has_events());
    }

    #[test]
    fn test_event_scheduling() {
        let mut sim = Simulator::new();
        
        // Schedule events at different times
        sim.schedule_event(
            SimulationTime::new(10),
            EventType::ProcessStart { machine_id: 0, process_id: 1 },
        );
        sim.schedule_event(
            SimulationTime::new(5),
            EventType::MaterialArrival { material_id: 1 },
        );
        
        // The earliest event should pop first (min-heap behavior)
        assert_eq!(sim.has_events(), true);
        
        let event = sim.step();
        assert!(event.is_some());
        // Should get the event from time 5 first
        assert_eq!(sim.current_time.as_minutes(), 5);
    }

    #[test]
    fn test_simulation_loop() {
        let mut sim = Simulator::new();
        
        // Schedule some events
        sim.schedule_event(
            SimulationTime::new(10),
            EventType::ProcessStart { machine_id: 0, process_id: 1 },
        );
        sim.schedule_event(
            SimulationTime::new(20),
            EventType::ProcessComplete { machine_id: 0, process_id: 1 },
        );
        
        let mut event_count = 0;
        sim.run_all(|_sim, event| {
            event_count += 1;
            println!("Event at time {}: {:?}", _sim.current_time.as_minutes(), event.event_type);
        });
        
        assert_eq!(event_count, 2);
        assert!(!sim.has_events());
    }
}
