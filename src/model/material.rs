pub struct Material {
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_keeps_name() {
        let material = Material {
            name: "Steel Sheet".to_string(),
        };
        assert_eq!(material.name, "Steel Sheet");
    }
}
