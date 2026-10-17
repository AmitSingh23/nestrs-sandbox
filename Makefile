.PHONY: up down logs build restart ps psql run clean

# Build images and start the app and Postgres in the background.
up:
	docker compose up -d --build

down:
	docker compose down

logs:
	docker compose logs -f

build:
	docker compose build

restart: down up

ps:
	docker compose ps

# Open a psql shell in the database container.
psql:
	docker compose exec db psql -U nestrs -d nestrs

# Run the app on the host against the Postgres container.
run:
	docker compose up -d db
	DATABASE_URL=postgres://nestrs:nestrs@localhost:5432/nestrs cargo run

# Stop everything and delete the database volume.
clean:
	docker compose down -v
