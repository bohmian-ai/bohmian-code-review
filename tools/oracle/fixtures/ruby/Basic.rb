require 'helper'

class Circle
  include Helper

  def initialize(radius)
    @radius = radius
  end

  def area
    self.compute(@radius)
  end
end
