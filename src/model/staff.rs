/// Represents a role that a staff member can have
/// Different roles may have different capabilities or costs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    pub id: u32,
    pub name: String,
    /// Some roles might be specialists that can only work on certain machines
    pub machine_ids: Vec<u32>,  // Empty = can work on any machine
}

impl Role {
    /// Create a new role
    pub fn new(id: u32, name: &str) -> Self {
        Role {
            id,
            name: name.to_string(),
            machine_ids: Vec::new(),
        }
    }

    /// Create a specialist role that can only work on specific machines
    pub fn specialist(id: u32, name: &str, machine_ids: Vec<u32>) -> Self {
        Role {
            id,
            name: name.to_string(),
            machine_ids,
        }
    }

    /// Check if this role can work on a specific machine
    pub fn can_work_on(&self, machine_id: u32) -> bool {
        if self.machine_ids.is_empty() {
            true  // No restrictions
        } else {
            self.machine_ids.contains(&machine_id)
        }
    }
}

/// Represents a staff member who can operate machines
#[derive(Debug, Clone)]
pub struct Staff {
    pub id: u32,
    pub name: String,
    pub role: Role,
    pub is_available: bool,
    /// Current machine they're working on (None if idle)
    pub current_machine: Option<u32>,
    /// Time they'll become available
    pub available_at: u32,
    /// Total minutes spent idle
    pub idle_time: u32,
    /// Last time availability changed (tracks idle accumulation)
    pub last_status_change: u32,
}

impl Staff {
    /// Create a new staff member
    pub fn new(id: u32, name: &str, role: Role) -> Self {
        Staff {
            id,
            name: name.to_string(),
            role,
            is_available: true,
            current_machine: None,
            available_at: 0,
            idle_time: 0,
            last_status_change: 0,
        }
    }

    /// Check if this staff member can work on a specific machine
    pub fn can_work_on(&self, machine_id: u32) -> bool {
        self.role.can_work_on(machine_id)
    }

    /// Assign this staff member to a machine
    /// Returns true if successfully assigned, false if busy
    pub fn assign_to_machine(&mut self, machine_id: u32, duration: u32, current_time: u32) -> bool {
        if self.is_available && self.can_work_on(machine_id) {
            // Accumulate idle time up to assignment
            self.idle_time += current_time.saturating_sub(self.last_status_change);
            self.is_available = false;
            self.current_machine = Some(machine_id);
            self.available_at = current_time + duration;
            self.last_status_change = current_time;
            true
        } else {
            false
        }
    }

    /// Release this staff member from a machine
    pub fn release_from_machine(&mut self, current_time: u32) {
        if current_time >= self.available_at {
            self.is_available = true;
            self.current_machine = None;
            self.last_status_change = current_time;
        }
    }

    /// Accumulate idle time up to a given simulation time
    pub fn accumulate_idle_until(&mut self, current_time: u32) {
        if self.is_available && current_time > self.last_status_change {
            self.idle_time += current_time - self.last_status_change;
            self.last_status_change = current_time;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_creation() {
        let role = Role::new(0, "Operator");
        assert_eq!(role.name, "Operator");
        assert_eq!(role.id, 0);
        assert!(role.machine_ids.is_empty());
    }

    #[test]
    fn test_specialist_role() {
        let role = Role::specialist(1, "CNC Specialist", vec![0, 1]);
        assert_eq!(role.name, "CNC Specialist");
        assert!(role.can_work_on(0));
        assert!(role.can_work_on(1));
        assert!(!role.can_work_on(2));
    }

    #[test]
    fn test_staff_creation() {
        let role = Role::new(0, "Operator");
        let staff = Staff::new(0, "John", role);
        assert_eq!(staff.id, 0);
        assert_eq!(staff.name, "John");
        assert!(staff.is_available);
        assert_eq!(staff.current_machine, None);
        assert_eq!(staff.idle_time, 0);
    }

    #[test]
    fn test_staff_assignment() {
        let role = Role::new(0, "Operator");
        let mut staff = Staff::new(0, "John", role);

        // Assign to machine
        let success = staff.assign_to_machine(0, 10, 0);
        assert!(success);
        assert!(!staff.is_available);
        assert_eq!(staff.current_machine, Some(0));
        assert_eq!(staff.available_at, 10);
        assert_eq!(staff.idle_time, 0); // No idle accumulated before first assignment

        // Try to assign while busy (should fail)
        let success = staff.assign_to_machine(1, 10, 5);
        assert!(!success);
        assert_eq!(staff.current_machine, Some(0));  // Still on machine 0

        // Release after time passes
        staff.release_from_machine(10);
        assert!(staff.is_available);
        assert_eq!(staff.current_machine, None);
        staff.accumulate_idle_until(20);
        assert_eq!(staff.idle_time, 10); // Idle from 10 to 20
    }

    #[test]
    fn test_specialist_restriction() {
        let role = Role::specialist(0, "CNC Op", vec![0, 1]);
        let mut staff = Staff::new(0, "Jane", role);

        // Can work on machine 0
        let success = staff.assign_to_machine(0, 10, 0);
        assert!(success);

        // Release and try machine 2 (should fail)
        staff.release_from_machine(10);
        let success = staff.assign_to_machine(2, 10, 10);
        assert!(!success);
    }
}
