/// Represents a machine that can process items
/// Machines require staff to operate
#[derive(Debug, Clone)]
pub struct MachineType {
    pub id: u32,
    pub name: String,
    /// Number of staff members required to operate this machine
    pub staff_required: u32,
    /// Whether this machine can run without staff (fully automated)
    pub is_automated: bool,
}

impl MachineType {
    /// Create a new machine that requires staff
    pub fn new(id: u32, name: &str, staff_required: u32) -> Self {
        MachineType {
            id,
            name: name.to_string(),
            staff_required,
            is_automated: false,
        }
    }

    /// Create a fully automated machine (no staff needed)
    pub fn automated(id: u32, name: &str) -> Self {
        MachineType {
            id,
            name: name.to_string(),
            staff_required: 0,
            is_automated: true,
        }
    }

    /// Check if this machine needs staff
    pub fn needs_staff(&self) -> bool {
        !self.is_automated && self.staff_required > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_manual_machine() {
        let machine = MachineType::new(1, "Press", 2);
        assert_eq!(machine.id, 1);
        assert_eq!(machine.name, "Press");
        assert_eq!(machine.staff_required, 2);
        assert!(machine.needs_staff());
        assert!(!machine.is_automated);
    }

    #[test]
    fn creates_automated_machine() {
        let machine = MachineType::automated(2, "Conveyor");
        assert_eq!(machine.id, 2);
        assert_eq!(machine.name, "Conveyor");
        assert_eq!(machine.staff_required, 0);
        assert!(machine.is_automated);
        assert!(!machine.needs_staff());
    }

    #[test]
    fn detects_staff_not_required_when_zero() {
        let machine = MachineType {
            staff_required: 0,
            ..MachineType::new(3, "Buffer", 0)
        };
        assert!(!machine.needs_staff());
    }
}
