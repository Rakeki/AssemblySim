/// Staff Scheduling System - How staff operates machines in the simulation
/// 
/// This module demonstrates:
/// - Assigning staff to machines
/// - Staff availability tracking
/// - Constraints (staff skills, availability)
/// - Bottleneck detection (waiting for staff)

use crate::model::time::{Simulator, SimulationTime, EventType};
use crate::model::staff::{Staff, Role};
use crate::model::machine::MachineType;

/// Represents a machine in operation with its current state
#[derive(Debug, Clone)]
pub struct MachineState {
    pub machine: MachineType,
    pub is_operating: bool,
    pub assigned_staff: Vec<u32>,  // IDs of staff working on this machine
    pub waiting_for: Option<String>,
    pub idle_time: u32,
    pub last_status_change: u32,
}

impl MachineState {
    pub fn new(machine: MachineType) -> Self {
        MachineState {
            machine,
            is_operating: false,
            assigned_staff: Vec::new(),
            waiting_for: None,
            idle_time: 0,
            last_status_change: 0,
        }
    }
}

/// Complete production simulation with staff scheduling
pub struct ProductionSimulator {
    pub simulator: Simulator,
    pub machines: Vec<MachineState>,
    pub staff: Vec<Staff>,
}

impl ProductionSimulator {
    pub fn new() -> Self {
        ProductionSimulator {
            simulator: Simulator::new(),
            machines: Vec::new(),
            staff: Vec::new(),
        }
    }

    /// Add a staff member to the production line
    pub fn add_staff(&mut self, staff: Staff) {
        self.staff.push(staff);
    }

    /// Add a machine to the production line
    pub fn add_machine(&mut self, machine: MachineType) {
        self.machines.push(MachineState::new(machine));
    }

    /// Try to start a process on a machine
    /// Returns true if successful, false if staff unavailable
    pub fn try_start_process(
        &mut self,
        machine_id: u32,
        process_id: u32,
        duration: u32,
        current_time: u32,
    ) -> bool {
        // Find the machine
        let machine = match self.machines.get_mut(machine_id as usize) {
            Some(m) => m,
            None => return false,
        };

        // If automated, start immediately
        if machine.machine.is_automated {
            machine.idle_time += current_time.saturating_sub(machine.last_status_change);
            machine.last_status_change = current_time;
            machine.is_operating = true;
            machine.waiting_for = None;
            // Schedule completion
            self.simulator.schedule_event(
                SimulationTime::new(current_time + duration),
                EventType::ProcessComplete {
                    machine_id,
                    process_id,
                },
            );
            return true;
        }

        // Find available staff
        let staff_needed = machine.machine.staff_required as usize;
        let mut available_staff = Vec::new();

        for (staff_idx, staff_member) in self.staff.iter().enumerate() {
            if staff_member.is_available && staff_member.can_work_on(machine_id) {
                available_staff.push(staff_idx);
                if available_staff.len() >= staff_needed {
                    break;
                }
            }
        }

        // Not enough staff available
        if available_staff.len() < staff_needed {
            self.simulator.schedule_event(
                SimulationTime::new(current_time),
                EventType::StaffUnavailable {
                    machine_id,
                    process_id,
                },
            );
            machine.waiting_for = Some("Staff".to_string());
            return false;
        }

        // Assign staff
        machine.idle_time += current_time.saturating_sub(machine.last_status_change);
        machine.last_status_change = current_time;
        machine.is_operating = true;
        machine.waiting_for = None;
        for staff_idx in available_staff {
            let staff_id = self.staff[staff_idx].id;
            self.staff[staff_idx].assign_to_machine(machine_id, duration, current_time);
            machine.assigned_staff.push(staff_id);

            // Schedule staff release event
            self.simulator.schedule_event(
                SimulationTime::new(current_time + duration),
                EventType::StaffReleased {
                    staff_id,
                    machine_id,
                },
            );
        }

        // Schedule process completion
        self.simulator.schedule_event(
            SimulationTime::new(current_time + duration),
            EventType::ProcessComplete {
                machine_id,
                process_id,
            },
        );

        true
    }

    /// Get a summary of current state
    pub fn get_status(&self) -> String {
        let mut status = format!("Production Status at time {}\n", self.simulator.elapsed_time());
        status.push_str("Machines:\n");
        for machine in &self.machines {
            let operating = if machine.is_operating { "Operating" } else { "Idle" };
            let waiting = machine
                .waiting_for
                .as_deref()
                .unwrap_or(if machine.is_operating { "" } else { "Next task" });
            status.push_str(&format!(
                "  - {} (ID: {}): {} with {} staff{} | Idle: {} mins\n",
                machine.machine.name,
                machine.machine.id,
                operating,
                machine.assigned_staff.len(),
                if waiting.is_empty() {
                    "".to_string()
                } else {
                    format!(" | Waiting for {}", waiting)
                },
                machine.idle_time
            ));
        }
        status.push_str("Staff:\n");
        for staff_member in &self.staff {
            let availability = if staff_member.is_available { "Available" } else { "Busy" };
            let machine_info = staff_member
                .current_machine
                .map(|m| format!("on machine {}", m))
                .unwrap_or_else(|| "idle".to_string());
            status.push_str(&format!(
                "  - {} (ID: {}): {} ({}) | Idle: {} mins\n",
                staff_member.name, staff_member.id, availability, machine_info, staff_member.idle_time
            ));
        }
        status
    }

    /// Update idle time for all available staff up to the provided time
    pub fn finalize_idle_time(&mut self, current_time: u32) {
        for staff in &mut self.staff {
            // Force-release staff whose expected end time has passed
            if !staff.is_available && current_time >= staff.available_at {
                staff.release_from_machine(current_time);
            }

            // If staff is marked busy but their machine isn't running or doesn't reference them, free them
            if !staff.is_available {
                if let Some(machine_id) = staff.current_machine {
                    match self.machines.get(machine_id as usize) {
                        Some(machine) => {
                            let still_assigned = machine.assigned_staff.contains(&staff.id);
                            if !machine.is_operating || !still_assigned {
                                staff.release_from_machine(current_time);
                            }
                        }
                        None => {
                            // Machine missing; free the staff
                            staff.release_from_machine(current_time);
                        }
                    }
                }
            }

            staff.accumulate_idle_until(current_time);
        }
        for machine in &mut self.machines {
            if !machine.is_operating {
                // If machine is idle but still has staff assigned, free them
                if !machine.assigned_staff.is_empty() {
                    for staff_id in machine.assigned_staff.drain(..) {
                        if let Some(staff_member) = self.staff.iter_mut().find(|s| s.id == staff_id) {
                            staff_member.release_from_machine(current_time);
                        }
                    }
                }
                if current_time > machine.last_status_change {
                    machine.idle_time += current_time - machine.last_status_change;
                    machine.last_status_change = current_time;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automated_machine() {
        let mut prod = ProductionSimulator::new();

        // Add automated machine (no staff needed)
        let auto_machine = MachineType::automated(0, "Conveyor Belt");
        prod.add_machine(auto_machine);

        // Start process without staff
        let success = prod.try_start_process(0, 0, 10, 0);
        assert!(success);
        assert!(prod.machines[0].is_operating);
        assert_eq!(prod.machines[0].assigned_staff.len(), 0);  // No staff assigned
    }

    #[test]
    fn test_machine_with_staff() {
        let mut prod = ProductionSimulator::new();

        // Add machine that needs 1 staff member
        let machine = MachineType::new(0, "CNC Machine", 1);
        prod.add_machine(machine);

        // Add a staff member
        let role = Role::new(0, "Operator");
        let staff = Staff::new(0, "John", role);
        prod.add_staff(staff);

        // Try to start process
        let success = prod.try_start_process(0, 0, 10, 0);
        assert!(success);
        assert!(prod.machines[0].is_operating);
        assert_eq!(prod.machines[0].assigned_staff.len(), 1);
        assert!(!prod.staff[0].is_available);  // Staff now busy
    }

    #[test]
    fn test_staff_unavailable() {
        let mut prod = ProductionSimulator::new();

        // Add machine that needs 2 staff members
        let machine = MachineType::new(0, "Press", 2);
        prod.add_machine(machine);

        // Add only 1 staff member
        let role = Role::new(0, "Operator");
        let staff = Staff::new(0, "John", role);
        prod.add_staff(staff);

        // Try to start process (should fail - need 2, have 1)
        let success = prod.try_start_process(0, 0, 10, 0);
        assert!(!success);
        assert!(!prod.machines[0].is_operating);
    }

    #[test]
    fn test_specialist_staff() {
        let mut prod = ProductionSimulator::new();

        // Add two machines
        let machine_a = MachineType::new(0, "CNC", 1);
        let machine_b = MachineType::new(1, "Lathe", 1);
        prod.add_machine(machine_a);
        prod.add_machine(machine_b);

        // Add specialist who can only work on CNC (machine 0)
        let role = Role::specialist(0, "CNC Specialist", vec![0]);
        let specialist = Staff::new(0, "Jane", role);
        prod.add_staff(specialist);

        // Should succeed on machine 0
        let success_a = prod.try_start_process(0, 0, 10, 0);
        assert!(success_a);

        // Reset and try machine 1 (should fail - specialist can't work there)
        prod.staff[0].release_from_machine(10);
        let success_b = prod.try_start_process(1, 0, 10, 10);
        assert!(!success_b);
    }

    #[test]
    fn test_multiple_sequential_processes() {
        let mut prod = ProductionSimulator::new();

        // Add machine and staff
        let machine = MachineType::new(0, "Welding Station", 1);
        prod.add_machine(machine);

        let role = Role::new(0, "Welder");
        let staff = Staff::new(0, "Bob", role);
        prod.add_staff(staff);

        // Process 1: Time 0-10
        let success1 = prod.try_start_process(0, 0, 10, 0);
        assert!(success1);
        assert!(!prod.staff[0].is_available);

        // Release staff at time 10
        prod.staff[0].release_from_machine(10);

        // Process 2: Time 10-20 (same staff)
        let success2 = prod.try_start_process(0, 1, 10, 10);
        assert!(success2);
        assert!(!prod.staff[0].is_available);
        assert_eq!(prod.staff[0].available_at, 20);
    }

    #[test]
    fn test_full_simulation_with_staff() {
        let mut prod = ProductionSimulator::new();

        // Create production line
        let machine = MachineType::new(0, "Assembly", 1);
        prod.add_machine(machine);

        let role = Role::new(0, "Assembler");
        let staff = Staff::new(0, "Alice", role);
        prod.add_staff(staff);

        // Schedule 3 items to be processed
        for item_id in 0..3 {
            let current_time = item_id * 15;
            prod.try_start_process(0, item_id, 10, current_time);

            // Release staff for next item
            if item_id < 2 {
                prod.staff[0].release_from_machine((item_id + 1) * 15);
            }
        }

        // Run simulation
        let mut event_count = 0;
        prod.simulator.run_all(|sim, event| {
            event_count += 1;
            println!(
                "Time {}: {:?}",
                sim.elapsed_time(),
                event.event_type
            );
        });

        // We should have events for each process
        assert!(event_count > 0);
    }

    #[test]
    fn test_staff_status() {
        let mut prod = ProductionSimulator::new();

        let machine = MachineType::new(0, "Machine A", 1);
        prod.add_machine(machine);

        let role = Role::new(0, "Worker");
        let staff = Staff::new(0, "Tom", role);
        prod.add_staff(staff);

        // Get status before and after assignment
        let _status_before = prod.get_status();
        prod.try_start_process(0, 0, 10, 0);
        let _status_after = prod.get_status();

        // Both should contain information (not crash)
        assert!(!_status_before.is_empty());
        assert!(!_status_after.is_empty());
    }

    #[test]
    fn finalize_idle_time_releases_stuck_staff() {
        let mut prod = ProductionSimulator::new();
        let machine = MachineType::new(0, "Machine A", 1);
        prod.add_machine(machine);

        let role = Role::new(0, "Operator");
        let staff = Staff::new(0, "Op", role);
        prod.add_staff(staff);

        // Start process at time 0
        assert!(prod.try_start_process(0, 0, 10, 0));
        // Simulate machine being idle while staff still attached
        prod.machines[0].is_operating = false;
        // Finalize after the original duration should free staff and count idle time
        prod.finalize_idle_time(15);

        assert!(prod.staff[0].is_available);
        assert_eq!(prod.staff[0].current_machine, None);
        assert_eq!(prod.machines[0].assigned_staff.len(), 0);
        assert_eq!(prod.machines[0].idle_time, 15);

        // Advance further to accumulate idle time for staff
        prod.finalize_idle_time(20);
        assert_eq!(prod.staff[0].idle_time, 5);
    }
}
