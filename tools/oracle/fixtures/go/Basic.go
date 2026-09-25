package demo

import "math"

type Circle struct {
	Radius float64
}

func (c Circle) Area() float64 {
	return math.Pi * c.Radius * c.Radius
}

func Make(radius float64) Circle {
	return Circle{Radius: radius}
}
