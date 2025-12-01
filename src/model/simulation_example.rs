/// This module shows practical examples of using the time simulation system
/// It demonstrates how to track machine availability, process completion, etc.

use super::time::{Simulator, SimulationTime, EventType};

/// Example: Simulate a machine processing items
/// 
/// Scenario:
/// - Machine 1 processes items that take 10 minutes each
/// - We want to schedule 3 items to be processed
/// - We want to track when each item is done
pub struct MachineSimulator {
    pub simulator: Simulator,
    pub machine_id: u32,
}

impl MachineSimulator {
    pub fn new(machine_id: u32) -> Self {
        MachineSimulator {
            simulator: Simulator::new(),
            machine_id,
        }
    }

    /// Schedule items to be processed on this machine
    /// Each item takes `process_time` minutes
    pub fn schedule_batch(&mut self, num_items: u32, process_time: u32) {
        let mut current_time = 0;
        
        for item_id in 0..num_items {
            let start_time = SimulationTime::new(current_time);
            let end_time = SimulationTime::new(current_time + process_time);

            // Schedule the start event
            self.simulator.schedule_event(
                start_time,
                EventType::ProcessStart {
                    machine_id: self.machine_id,
                    process_id: item_id,
                },
            );

            // Schedule the completion event
            self.simulator.schedule_event(
                end_time,
                EventType::ProcessComplete {
                    machine_id: self.machine_id,
                    process_id: item_id,
                },
            );

            // Next item starts when this one finishes
            current_time += process_time;
        }
    }

    /// Get the total time it takes to process all items
    pub fn total_time_minutes(&self) -> u32 {
        self.simulator.elapsed_time()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Example test showing practical simulation usage
    #[test]
    fn test_machine_processing_batch() {
        let mut machine = MachineSimulator::new(0);
        
        // Process 3 items, each taking 10 minutes
        machine.schedule_batch(3, 10);

        let mut events = Vec::new();
        machine.simulator.run_all(|sim, event| {
            events.push((sim.elapsed_time(), event.event_type.clone()));
        });

        // We should have 6 events (3 starts + 3 completions)
        assert_eq!(events.len(), 6);

        // Verify timeline:
        // Item 0: starts at 0, completes at 10
        // Item 1: starts at 10, completes at 20
        // Item 2: starts at 20, completes at 30
        assert_eq!(events[0].0, 0);   // First start
        assert_eq!(events[1].0, 10);  // First complete / Second start
        assert_eq!(events[2].0, 10);  // Second start
        assert_eq!(events[3].0, 20);  // Second complete / Third start
        assert_eq!(events[4].0, 20);  // Third start
        assert_eq!(events[5].0, 30);  // Third complete

        println!("Machine finished all items at time: {} minutes", 
                 machine.simulator.elapsed_time());
    }

    /// This demonstrates how to calculate bottlenecks
    #[test]
    fn test_multiple_machines_timeline() {
        let mut machine_a = MachineSimulator::new(0);
        let mut machine_b = MachineSimulator::new(1);

        // Machine A: 3 items × 10 minutes each
        machine_a.schedule_batch(3, 10);

        // Machine B: 2 items × 15 minutes each
        machine_b.schedule_batch(2, 15);

        // Run the simulations to advance time
        machine_a.simulator.run_all(|_sim, _event| {});
        machine_b.simulator.run_all(|_sim, _event| {});

        let time_a = machine_a.simulator.elapsed_time();
        let time_b = machine_b.simulator.elapsed_time();

        println!("Machine A finishes at: {} minutes", time_a);
        println!("Machine B finishes at: {} minutes", time_b);

        // Machine A: 3 × 10 = 30 minutes
        // Machine B: 2 × 15 = 30 minutes
        assert_eq!(time_a, 30);
        assert_eq!(time_b, 30);
    }
}
