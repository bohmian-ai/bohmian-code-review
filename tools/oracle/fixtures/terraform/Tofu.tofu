variable "app_name" {
  type = string
}

resource "null_resource" "app" {
  triggers = {
    name = var.app_name
  }
}
