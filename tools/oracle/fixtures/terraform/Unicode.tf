locals {
  greeting = "héllo wörld 😀"
  name     = "café"
}

resource "aws_instance" "démo" {
  count         = 1
  instance_type = "t3.micro"
  tags = {
    Name = local.name
    Note = local.greeting
  }
}
