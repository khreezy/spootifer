docker kill spootifer && docker run --detach --mount type=volume,src=spootifer-local,target=/etc/db -p 8080:8081 --env-file .env --name spootifer --rm spootifer