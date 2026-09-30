const { helper } = require('./util');

function area(radius) {
  return helper(radius);
}

class Circle {
  constructor(radius) {
    this.radius = radius;
  }
}
