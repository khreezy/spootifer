docker rm -f spootifer && docker run --name spootifer --mount type=volume,volume-driver=local,src=spootifer-local,dst=/db --env-file .env -dp 8080:8081  spootifer:latest