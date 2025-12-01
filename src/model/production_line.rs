use super::material::Material;

pub struct ProductionLine {
    pub name: String,
    pub start_material: Material,
    pub end_material: Material,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_line_links_materials() {
        let start = Material {
            name: "Raw Steel".to_string(),
        };
        let end = Material {
            name: "Chassis".to_string(),
        };
        let line = ProductionLine {
            name: "Chassis Line".to_string(),
            start_material: start,
            end_material: end,
        };

        assert_eq!(line.name, "Chassis Line");
        assert_eq!(line.start_material.name, "Raw Steel");
        assert_eq!(line.end_material.name, "Chassis");
    }
}
