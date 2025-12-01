use super::material::Material;

pub struct ProductionLine {
    pub name: String,
    pub start_material: Material,
    pub end_material: Material,
}