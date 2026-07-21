# soaricarus_auth
A auth web API services for the soaricarus project.


# Getting Started
Install the `sqlx` tool to use migrations.
```
cargo install sqlx-cli
```
This will be used to scaffold development for local environments.


The easiest way to get started is through docker. This assumes that docker is already installed
on your system. Copy the `.env.docker.sample` as `.env`. Most of the data in the env file doesn't 
need to be modified. The `SECRET_KEY` variable should be changed since it will be used for token
generation. The `SECRET_PASSPHASE` should also be changed when in production mode, but make sure
the respective `passphrase` database table record exists.

To enable or disable registrations, use `TRUE` or `FALSE` for the `ENABLE_REGISTRATION` variable.
By default it is `TRUE`.


### Build image
```
docker compose build
```

### Start images
```
docker compose up -d --force-recreate
```

### Bring it down
```
docker compose down -v
```

### Pruning
```
docker system prune -a
```

To view the OpenAPI spec, run the project and access `/swagger-ui`. If running through docker,
the url would be something like `http://localhost:8001/swagger-ui`.
