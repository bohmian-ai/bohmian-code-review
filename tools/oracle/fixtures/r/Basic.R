library(stats)

Circle <- setRefClass("Circle",
  fields = list(radius = "numeric"),
  methods = list(
    area = function() {
      pi * radius^2
    }
  )
)

make <- function(radius) {
  Circle$new(radius = radius)
}
