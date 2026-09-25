#include <cmath>

class Circle {
public:
    explicit Circle(double radius) : radius_(radius) {}
    double area() const { return M_PI * radius_ * radius_; }
private:
    double radius_;
};
