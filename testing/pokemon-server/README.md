# Pokemon Benchmark Server

A blazingly fast Axum API server serving all 151 original Pokemon.

## Endpoints

- `GET /api/pokemon` - List all 151 Pokemon
- `GET /api/pokemon/{id}` - Get Pokemon by ID (1-151)
- `GET /api/pokemon/type/{type}` - Filter by type (normal, fire, water, etc.)
- `GET /health` - Health check

## Run

```bash
cargo run --bin pokemon-server
```

Server starts on http://127.0.0.1:8080
