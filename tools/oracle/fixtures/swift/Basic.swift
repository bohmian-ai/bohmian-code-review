import Foundation

class Circle: Shape {
    let radius: Double

    init(radius: Double) {
        self.radius = radius
    }

    func area() -> Double {
        return Double.pi * radius * radius
    }
}
