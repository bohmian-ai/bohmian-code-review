local helper = require("util")

export type Circle = {
  radius: number,
}

local function make(radius: number): Circle
  return { radius = radius }
end

local function area(c: Circle): number
  return helper.compute(c.radius)
end

return {
  make = make,
  area = area,
}
