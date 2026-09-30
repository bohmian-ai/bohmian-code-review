<?php

namespace Demo;

class Circle implements Shape
{
    private float $radius;

    public function __construct(float $radius)
    {
        $this->radius = $radius;
    }

    public function area(): float
    {
        return M_PI * $this->radius * $this->radius;
    }
}
