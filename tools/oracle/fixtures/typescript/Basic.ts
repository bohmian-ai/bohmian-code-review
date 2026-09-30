import { helper } from './util';

export interface Shape {
  area(): number;
}

export class Circle implements Shape {
  radius: number;
  constructor(radius: number) {
    this.radius = radius;
  }
  area(): number {
    return helper(this.radius);
  }
}
