use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use rustdata_core::{CrudRepository, backends::Sqlite, Entity};
use rustdata_migrations::{Transpiler, Dialect, TranspileOutput};
use serde::{Deserialize, Serialize};

// Define our User entity using the Entity macro
#[derive(Debug, Clone, Serialize, Deserialize, Entity)]
#[entity(table = "users", order_by = "created_at DESC")]
struct User {
    #[entity(id)]
    id: uuid::Uuid,
    
    #[entity(column = "username")]
    username: String,
    
    #[entity(column = "email")]
    email: String,
    
    #[entity(auto_generated)]
    created_at: chrono::DateTime<chrono::Utc>,
}

/// Run migrations using the transpiler to convert canonical SQL to SQLite
async fn run_migrations(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let transpiler = Transpiler::new(Dialect::Sqlite);
    
    // Define migration SQL in canonical (Postgres-like) format
    let migrations: Vec<&str> = vec![
        // Migration 1: Create users table
        r#"
-- @migration V1
-- @description Create users table
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#,
    ];
    
    for migration in migrations {
        // Transpile to SQLite dialect
        let TranspileOutput { sql, .. } = transpiler.transpile(migration)?;
        
        // Execute the transpiled SQL
        sqlx::query(&sql).execute(pool).await?;
        println!("Migration executed successfully");
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create in-memory SQLite database
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await?;
    
    // Run migrations to create the users table
    run_migrations(&pool).await?;
    
    // Create repository instance
    let user_repo = CrudRepository::<Sqlite, User>::new(pool);

    // Create a new user
    let new_user = User {
        id: uuid::Uuid::new_v4(),
        username: "john_doe".to_string(),
        email: "john@example.com".to_string(),
        created_at: chrono::Utc::now(),
    };

    // Insert user
    let inserted = user_repo.insert(new_user).await?;
    println!("Created user: {:?}", inserted);

    // Find user by ID
    if let Some(found_user) = user_repo.find_by_id(inserted.id.clone()).await? {
        println!("Found user: {:?}", found_user);
    }

    // Update user
    let mut updated_user = inserted.clone();
    updated_user.email = "john.doe@example.com".to_string();
    let updated = user_repo.update(updated_user).await?;
    println!("Updated user: {:?}", updated);

    // Find all users
    let all_users = user_repo.find_all().await?;
    println!("All users: {:?}", all_users);

    // Delete user
    user_repo.delete(&updated.id).await?;
    println!("User deleted");

    Ok(())
}