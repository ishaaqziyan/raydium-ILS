doppler_project := "raydium-il-simulator"
doppler_config := "dev"

# list recipes
default:
    @just --list

# run the backend (Axum, :3001) with secrets from Doppler
backend:
    cd backend && doppler run --project {{doppler_project}} --config {{doppler_config}} -- cargo run

# run the frontend dev server (Astro, :4321)
frontend:
    cd frontend && npx astro dev

# stop the backgrounded astro dev server
frontend-stop:
    cd frontend && npx astro dev stop

# run backend unit tests (il_calc.rs)
test:
    cd backend && cargo test

# type-check the frontend (astro + ts)
check:
    cd frontend && npx astro check

# cargo build, backend
build-backend:
    cd backend && cargo build

# astro build, frontend
build-frontend:
    cd frontend && npx astro build

# fmt + clippy on the backend
lint:
    cd backend && cargo fmt --check && cargo clippy -- -D warnings

# install frontend deps (first-time setup)
install:
    cd frontend && npm install
