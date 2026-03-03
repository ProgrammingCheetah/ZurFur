set dotenv-load := true

default:
    @just --list

up:
    docker compose up -d

down:
    docker compose down

logs:
    docker compose logs -f

dev-back:
    cd backend && cargo watch -x run

dev-front:
    cd frontend && npm run dev

check:
    cd backend && bacon

db-shell:
    docker compose exec db psql -U admin -d zurfur

migrate-add name:
    cd backend && sqlx migrate add -r {{ name }}

migrate-run:
    cd backend && sqlx migrate run

migrate-revert:
    cd backend && sqlx migrate revert

db-reset:
    cd backend && sqlx database drop -y
    cd backend && sqlx database create
    cd backend && sqlx migrate run

setup-tools:
    cargo install cargo-watch sqlx-cli bacon
    npm install -g pnpm

clean:
    cargo clean
    rm -rf frontend/node_modules
