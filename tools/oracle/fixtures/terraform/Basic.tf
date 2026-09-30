terraform {
  required_version = ">= 1.5"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

variable "region" {
  type    = string
  default = "us-east-1"
}

variable "instance_type" {
  type = string
}

locals {
  name_prefix = "web"
  common_tags = {
    Environment = var.environment
  }
}

provider "aws" {
  region = var.region
  alias  = "west"
}

resource "aws_s3_bucket" "data" {
  bucket = "${local.name_prefix}-data"
  tags   = local.common_tags
}

resource "aws_instance" "web" {
  provider      = aws.west
  instance_type = var.instance_type
  ami           = data.aws_ami.ubuntu.id
}

data "aws_ami" "ubuntu" {
  most_recent = true
  owners      = ["099720109477"]
}

module "vpc" {
  source   = "./modules/vpc"
  cidr     = var.region
  providers = {
    aws = aws.west
  }
}

output "bucket_arn" {
  value = aws_s3_bucket.data.arn
}

moved {
  from = aws_instance.old
  to   = aws_instance.web
}
