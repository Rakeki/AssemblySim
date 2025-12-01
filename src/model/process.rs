use super::machine::MachineType;

pub struct Process {
    pub required_machine: Vec<MachineType>,
    pub time_per_unit: u32,
}