use super::machine::MachineType;

pub struct Process {
    pub required_machine: Vec<MachineType>,
    pub time_per_unit: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_carries_required_machines_and_duration() {
        let welder = MachineType::new(0, "Welder", 1);
        let assembler = MachineType::new(1, "Assembler", 2);
        let process = Process {
            required_machine: vec![welder.clone(), assembler.clone()],
            time_per_unit: 15,
        };

        assert_eq!(process.time_per_unit, 15);
        assert_eq!(process.required_machine.len(), 2);
        assert_eq!(process.required_machine[0].name, welder.name);
        assert_eq!(process.required_machine[1].staff_required, assembler.staff_required);
    }
}
