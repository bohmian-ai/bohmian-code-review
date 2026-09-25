local helper = require("util")

local Circle = {}
Circle.__index = Circle

function Circle.new(radius)
  local self = setmetatable({}, Circle)
  self.radius = radius
  return self
end

function Circle:area()
  return helper.compute(self.radius)
end

return Circle
