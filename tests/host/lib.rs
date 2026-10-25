extern crate self as rs_matter_embassy;
pub use rs_matter as matter;

#[path = "../../src/light.rs"]
pub mod light;
#[path = "../../src/matter_light.rs"]
pub mod matter_light;
