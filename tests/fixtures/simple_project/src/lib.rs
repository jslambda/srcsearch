//! Simple fixture crate.

/// Adds one to the input.
pub fn add_one(value: i32) -> i32 {
    value + 1
}

/// A sample struct.
pub struct Widget {
    pub id: u32,
}

impl Widget {
    /// Return the widget id.
    pub fn id(&self) -> u32 {
        self.id
    }
}
