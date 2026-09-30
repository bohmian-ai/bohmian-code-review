mod util;

pub struct Circle {
    radius: f64,
}

impl Circle {
    pub fn new(radius: f64) -> Self {
        Circle { radius }
    }

    pub fn area(&self) -> f64 {
        util::helper(self.radius)
    }
}

pub fn make(radius: f64) -> Circle {
    Circle::new(radius)
}
