package demo

class Circle(private val radius: Double) : Shape {
    override fun area(): Double {
        return Math.PI * radius * radius
    }
}

fun make(radius: Double): Circle {
    return Circle(radius)
}
