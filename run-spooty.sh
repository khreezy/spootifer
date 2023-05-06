aws ssm get-parameter --with-decryption --name spootifer-env --region us-west-2 | jq -r '.Parameter.Value' > .env
docker compose up --force-recreate --build --detach