aws ssm get-parameter --with-decryption --name spootifer-env --region us-west-2 | jq '.Parameter.Value' > .env
docker compose up --detached