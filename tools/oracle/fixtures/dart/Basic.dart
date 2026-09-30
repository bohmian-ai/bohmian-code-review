import 'util.dart';

class Circle implements Shape {
  final double radius;

  Circle(this.radius);

  @override
  double area() {
    return helper(this.radius);
  }
}
