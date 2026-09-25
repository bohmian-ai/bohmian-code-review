package demo

class Circle(radius: Double) extends Shape {
  override def area: Double = {
    Math.PI * radius * radius
  }
}

object Circle {
  def make(radius: Double): Circle = new Circle(radius)
}
