#!/bin/bash
# LocalStack initialization for AWS Bedrock

# Wait for LocalStack to be ready
sleep 5

# Create S3 bucket for Bedrock (if needed)
awslocal s3 mb s3://bedrock-models 2>/dev/null || true

echo "LocalStack initialization complete"
