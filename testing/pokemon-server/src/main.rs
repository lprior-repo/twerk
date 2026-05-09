//! Blazingly Fast Pokemon API Server
//! 
//! Uses Axum for high-performance HTTP handling.
//! Serves all 151 original Pokemon with type filtering.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// POKEMON DATA - All 151 Original Pokemon
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pokemon {
    pub id: u8,
    pub name: String,
    pub types: Vec<String>,
    pub base_stats: Stats,
    pub generation: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub hp: u16,
    pub attack: u16,
    pub defense: u16,
    pub sp_attack: u16,
    pub sp_defense: u16,
    pub speed: u16,
}

impl Stats {
    pub fn total(&self) -> u32 {
        self.hp as u32 + self.attack as u32 + self.defense as u32 
            + self.sp_attack as u32 + self.sp_defense as u32 + self.speed as u32
    }
}

include!("pokemon_data.rs");

// In-memory Pokemon store
#[derive(Clone)]
pub struct PokemonStore {
    pub pokemon: Arc<Vec<Pokemon>>,
    pub by_type: Arc<RwLock<std::collections::HashMap<String, Vec<u8>>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Pokemon not found: {0}")]
    NotFound(u8),
    
    #[error("Type not found: {0}")]
    TypeNotFound(String),
    
    #[error("Invalid Pokemon ID: {0} (must be 1-151)")]
    InvalidId(u8),
}

impl Default for PokemonStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PokemonStore {
    pub fn new() -> Self {
        let pokemon: Vec<Pokemon> = POKEMON_DATA.iter().map(|p| Pokemon {
            id: *p.0,
            name: p.1.to_string(),
            types: p.2.iter().map(|s| s.to_string()).collect(),
            base_stats: Stats {
                hp: p.3[0],
                attack: p.3[1],
                defense: p.3[2],
                sp_attack: p.3[3],
                sp_defense: p.3[4],
                speed: p.3[5],
            },
            generation: 1,
        }).collect();
        
        let mut by_type_map: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
        for p in &pokemon {
            for t in &p.types {
                by_type_map.entry(t.to_lowercase()).or_default().push(p.id);
            }
        }
        
        Self {
            pokemon: Arc::new(pokemon),
            by_type: Arc::new(RwLock::new(by_type_map)),
        }
    }
    
    pub fn get_all(&self) -> &[Pokemon] {
        &self.pokemon
    }
    
    pub fn get_by_id(&self, id: u8) -> Option<&Pokemon> {
        self.pokemon.iter().find(|p| p.id == id)
    }
    
    pub async fn get_by_type(&self, type_name: &str) -> Vec<u8> {
        let map = self.by_type.read().await;
        map.get(&type_name.to_lowercase()).cloned().unwrap_or_default()
    }
    
    /// Get Pokemon count
    pub fn len(&self) -> usize {
        self.pokemon.len()
    }
    
    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        self.pokemon.is_empty()
    }
    
    /// Validate Pokemon ID
    pub fn validate_id(id: u8) -> Result<(), StoreError> {
        if id == 0 || id > 151 {
            return Err(StoreError::InvalidId(id));
        }
        Ok(())
    }
}

// ============================================================================
// API HANDLERS
// ============================================================================

async fn get_all_pokemon(State(store): State<PokemonStore>) -> Json<Vec<Pokemon>> {
    Json(store.get_all().to_vec())
}

async fn get_pokemon_by_id(
    State(store): State<PokemonStore>,
    Path(id): Path<u8>,
) -> Response {
    match store.get_by_id(id) {
        Some(p) => Json(p).into_response(),
        None => (StatusCode::NOT_FOUND, "Pokemon not found").into_response(),
    }
}

async fn get_pokemon_by_type(
    State(store): State<PokemonStore>,
    Path(type_name): Path<String>,
) -> Json<Vec<u8>> {
    Json(store.get_by_type(&type_name).await)
}

async fn health_check() -> &'static str {
    "OK"
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    let store = PokemonStore::new();
    
    // Verify we loaded all 151 Pokemon
    assert_eq!(store.len(), 151, "Expected 151 Pokemon, got {}", store.len());
    
    let app = Router::new()
        .route("/api/pokemon", get(get_all_pokemon))
        .route("/api/pokemon/{id}", get(get_pokemon_by_id))
        .route("/api/pokemon/type/{pokemon_type}", get(get_pokemon_by_type))
        .route("/health", get(health_check))
        .with_state(store);
    
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    
    println!("🚀 Pokemon API Server running on http://127.0.0.1:8080");
    println!("📋 Endpoints:");
    println!("   GET /api/pokemon            - All 151 Pokemon");
    println!("   GET /api/pokemon/25          - Pikachu");
    println!("   GET /api/pokemon/type/fire   - Fire type Pokemon");
    println!("   GET /health                  - Health check");
    
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        eprintln!("❌ Failed to bind to {}: {}", addr, e);
        eprintln!("   Is port 8080 already in use?");
        e
    })?;
    
    println!("✅ Server ready to accept connections");
    
    axum::serve(listener, app).await.map_err(|e| {
        eprintln!("❌ Server error: {}", e);
        e
    })?;
    
    Ok(())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // PokemonStore Tests
    // ========================================================================

    #[test]
    fn test_pokemon_store_creation() {
        let store = PokemonStore::new();
        assert_eq!(store.len(), 151);
        assert!(!store.is_empty());
    }

    #[test]
    fn test_pokemon_store_get_all() {
        let store = PokemonStore::new();
        let all = store.get_all();
        assert_eq!(all.len(), 151);
        assert_eq!(all[0].name, "Bulbasaur");
        assert_eq!(all[150].name, "Mew");
    }

    #[test]
    fn test_pokemon_store_get_by_id_bulbasaur() {
        let store = PokemonStore::new();
        let bulbasaur = store.get_by_id(1);
        assert!(bulbasaur.is_some());
        let p = bulbasaur.unwrap();
        assert_eq!(p.name, "Bulbasaur");
        assert_eq!(p.types, vec!["Grass", "Poison"]);
        assert_eq!(p.generation, 1);
    }

    #[test]
    fn test_pokemon_store_get_by_id_pikachu() {
        let store = PokemonStore::new();
        let pikachu = store.get_by_id(25);
        assert!(pikachu.is_some());
        let p = pikachu.unwrap();
        assert_eq!(p.name, "Pikachu");
        assert_eq!(p.types, vec!["Electric"]);
    }

    #[test]
    fn test_pokemon_store_get_by_id_mew() {
        let store = PokemonStore::new();
        let mew = store.get_by_id(151);
        assert!(mew.is_some());
        let p = mew.unwrap();
        assert_eq!(p.name, "Mew");
        assert_eq!(p.base_stats.total(), 600);
    }

    #[test]
    fn test_pokemon_store_get_by_id_not_found() {
        let store = PokemonStore::new();
        assert!(store.get_by_id(0).is_none());
        assert!(store.get_by_id(152).is_none());
        assert!(store.get_by_id(255).is_none());
    }

    #[tokio::test]
    async fn test_pokemon_store_get_by_type_fire() {
        let store = PokemonStore::new();
        let fire_ids = store.get_by_type("fire").await;
        assert!(!fire_ids.is_empty());
        // Charizard should be in fire type
        assert!(fire_ids.contains(&6));
    }

    #[tokio::test]
    async fn test_pokemon_store_get_by_type_water() {
        let store = PokemonStore::new();
        let water_ids = store.get_by_type("water").await;
        assert!(!water_ids.is_empty());
        // Squirtle should be in water type
        assert!(water_ids.contains(&7));
    }

    #[tokio::test]
    async fn test_pokemon_store_get_by_type_grass() {
        let store = PokemonStore::new();
        let grass_ids = store.get_by_type("grass").await;
        assert!(!grass_ids.is_empty());
        // Bulbasaur should be in grass type
        assert!(grass_ids.contains(&1));
    }

    #[tokio::test]
    async fn test_pokemon_store_get_by_type_case_insensitive() {
        let store = PokemonStore::new();
        let fire_lower = store.get_by_type("fire").await;
        let fire_upper = store.get_by_type("FIRE").await;
        let fire_mixed = store.get_by_type("Fire").await;
        
        assert_eq!(fire_lower, fire_upper);
        assert_eq!(fire_lower, fire_mixed);
    }

    #[tokio::test]
    async fn test_pokemon_store_get_by_type_not_found() {
        let store = PokemonStore::new();
        let unknown_ids = store.get_by_type("unknown_type").await;
        assert!(unknown_ids.is_empty());
    }

    #[test]
    fn test_validate_id_valid() {
        assert!(PokemonStore::validate_id(1).is_ok());
        assert!(PokemonStore::validate_id(151).is_ok());
        assert!(PokemonStore::validate_id(75).is_ok());
    }

    #[test]
    fn test_validate_id_zero() {
        let result = PokemonStore::validate_id(0);
        assert!(result.is_err());
        match result {
            Err(StoreError::InvalidId(id)) => assert_eq!(id, 0),
            _ => panic!("Expected InvalidId error"),
        }
    }

    #[test]
    fn test_validate_id_above_151() {
        let result = PokemonStore::validate_id(152);
        assert!(result.is_err());
        match result {
            Err(StoreError::InvalidId(id)) => assert_eq!(id, 152),
            _ => panic!("Expected InvalidId error"),
        }
    }

    #[test]
    fn test_validate_id_255() {
        let result = PokemonStore::validate_id(255);
        assert!(result.is_err());
    }

    // ========================================================================
    // Stats Tests
    // ========================================================================

    #[test]
    fn test_stats_total_bulbasaur() {
        let store = PokemonStore::new();
        let bulbasaur = store.get_by_id(1).unwrap();
        assert_eq!(bulbasaur.base_stats.total(), 45 + 49 + 49 + 65 + 65 + 45);
    }

    #[test]
    fn test_stats_total_mewtwo() {
        let store = PokemonStore::new();
        let mewtwo = store.get_by_id(150).unwrap();
        // 106 + 110 + 90 + 154 + 90 + 130 = 680
        assert_eq!(mewtwo.base_stats.total(), 680);
    }

    #[test]
    fn test_stats_total_mew() {
        let store = PokemonStore::new();
        let mew = store.get_by_id(151).unwrap();
        // All stats are 100: 100 * 6 = 600
        assert_eq!(mew.base_stats.total(), 600);
    }

    #[test]
    fn test_stats_knockout_values() {
        let store = PokemonStore::new();
        let mewtwo = store.get_by_id(150).unwrap();
        assert_eq!(mewtwo.base_stats.hp, 106);
        assert_eq!(mewtwo.base_stats.attack, 110);
        assert_eq!(mewtwo.base_stats.defense, 90);
        assert_eq!(mewtwo.base_stats.sp_attack, 154);
        assert_eq!(mewtwo.base_stats.sp_defense, 90);
        assert_eq!(mewtwo.base_stats.speed, 130);
    }

    // ========================================================================
    // Pokemon Data Integrity Tests
    // ========================================================================

    #[test]
    fn test_all_151_pokemon_present() {
        let store = PokemonStore::new();
        for id in 1..=151 {
            assert!(store.get_by_id(id as u8).is_some(), "Missing Pokemon ID: {}", id);
        }
    }

    #[test]
    fn test_pokemon_names_unique() {
        let store = PokemonStore::new();
        let names: Vec<&str> = store.get_all().iter().map(|p| p.name.as_str()).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        sorted_names.dedup();
        assert_eq!(names.len(), sorted_names.len(), "Duplicate Pokemon names found");
    }

    #[test]
    fn test_pokemon_ids_unique() {
        let store = PokemonStore::new();
        let ids: Vec<u8> = store.get_all().iter().map(|p| p.id).collect();
        let mut sorted_ids = ids.clone();
        sorted_ids.sort();
        sorted_ids.dedup();
        assert_eq!(ids.len(), sorted_ids.len(), "Duplicate Pokemon IDs found");
    }

    #[test]
    fn test_all_ids_1_to_151() {
        let store = PokemonStore::new();
        let mut ids: Vec<u8> = store.get_all().iter().map(|p| p.id).collect();
        ids.sort();
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(*id, (i + 1) as u8, "ID sequence broken at position {}", i);
        }
    }

    #[test]
    fn test_all_generation_1() {
        let store = PokemonStore::new();
        for pokemon in store.get_all() {
            assert_eq!(pokemon.generation, 1, "Pokemon {} should be Gen 1", pokemon.name);
        }
    }

    // ========================================================================
    // StoreError Tests
    // ========================================================================

    #[test]
    fn test_store_error_display_not_found() {
        let err = StoreError::NotFound(200);
        assert!(err.to_string().contains("200"));
    }

    #[test]
    fn test_store_error_display_type_not_found() {
        let err = StoreError::TypeNotFound("fire".to_string());
        assert!(err.to_string().contains("fire"));
    }

    #[test]
    fn test_store_error_display_invalid_id() {
        let err = StoreError::InvalidId(0);
        let s = err.to_string();
        assert!(s.contains("0") || s.contains("Invalid"));
    }

    // ========================================================================
    // Pokemon Clone Tests
    // ========================================================================

    #[test]
    fn test_pokemon_clone() {
        let store = PokemonStore::new();
        let pikachu = store.get_by_id(25).unwrap().clone();
        assert_eq!(pikachu.name, "Pikachu");
        assert_eq!(pikachu.id, 25);
    }

    // ========================================================================
    // Response Type Tests
    // ========================================================================

    #[test]
    fn test_pokemon_serialization() {
        let store = PokemonStore::new();
        let pikachu = store.get_by_id(25).unwrap();
        let json = serde_json::to_string(pikachu).unwrap();
        assert!(json.contains("Pikachu"));
        assert!(json.contains("Electric"));
    }

    #[test]
    fn test_pokemon_deserialization() {
        let json = r#"{"id":25,"name":"Pikachu","types":["Electric"],"base_stats":{"hp":35,"attack":55,"defense":40,"sp_attack":50,"sp_defense":50,"speed":90},"generation":1}"#;
        let pokemon: Pokemon = serde_json::from_str(json).unwrap();
        assert_eq!(pokemon.name, "Pikachu");
    }

    // ========================================================================
    // Type Coverage Tests
    // ========================================================================

    #[tokio::test]
    async fn test_all_types_present() {
        let store = PokemonStore::new();
        let types = [
            "fire", "water", "grass", "electric", "psychic",
            "bug", "normal", "poison", "ground", "rock",
            "ghost", "ice", "fighting", "dragon", "flying"
        ];
        
        for type_name in types {
            let ids = store.get_by_type(type_name).await;
            assert!(!ids.is_empty(), "Type '{}' should have at least one Pokemon", type_name);
        }
    }

    #[tokio::test]
    async fn test_legendary_pokemon() {
        let store = PokemonStore::new();
        
        // Articuno
        let articuno = store.get_by_type("ice").await;
        assert!(articuno.contains(&144));
        
        // Zapdos
        let zapdos = store.get_by_type("electric").await;
        assert!(zapdos.contains(&145));
        
        // Moltres
        let moltres = store.get_by_type("fire").await;
        assert!(moltres.contains(&146));
        
        // Mewtwo
        let mewtwo = store.get_by_id(150).unwrap();
        assert_eq!(mewtwo.name, "Mewtwo");
        
        // Dragonite
        let dragonite = store.get_by_id(149).unwrap();
        assert!(dragonite.types.contains(&"Dragon".to_string()));
    }
}
