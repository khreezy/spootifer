docker compose down
aws ssm get-parameter --with-decryption --name "$ENV_PARAM_NAME" --region us-west-2 | jq -r '.Parameter.Value' > .env
docker compose pull
docker compose up --force-recreate --build --detach