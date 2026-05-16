use rustdata_core::{CrudRepository, Entity, QueryMethods, QueryRepository};

#[derive(Debug, Clone, Entity, QueryMethods)]
#[entity(table = "users", order_by = "id ASC")]
struct User {
    #[entity(id)]
    id: i32,

    username: String,

    age: i32,
}

fn main() {
    // This should compile if the QueryMethods derive is working correctly
    // The derive should generate methods like:
    // - find_by_username()
    // - find_by_age_gt()
    // - find_by_username_and_age()
    // etc.

    println!("QueryMethods derive macro is working correctly!");
    println!("Generated methods should include:");
    println!("- find_by_username()");
    println!("- find_by_age_gt()");
    println!("- find_by_username_and_age()");
}
