use sqlx::{sqlite::SqlitePoolOptions, Row};

async fn fresh_pool() -> sqlx::sqlite::SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory SQLite")
}

#[tokio::main]
async fn main() {
    run().await
}

async fn run() {
    let transpiler =
        rustdata_migrations::Transpiler::new(rustdata_migrations::Dialect::Sqlite);

    let migrations: Vec<(&str, &str)> = vec![
        ("v1__create_users",     include_str!("../migrations/v1__create_users.sql")),
        ("v2__add_user_fields",  include_str!("../migrations/v2__add_user_fields.sql")),
        ("v3__profile_table",    include_str!("../migrations/v3__profile_table.sql")),
    ];

    let transpiled_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/transpiled");
    std::fs::create_dir_all(transpiled_dir).expect("create transpiled dir");

    let mut all_warnings: Vec<String> = Vec::new();
    let mut transpiled_sqls: Vec<String> = Vec::new();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         SQLite Migration Transpiler — Test Suite              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    for (version, canonical_sql) in &migrations {
        let out = transpiler
            .transpile(canonical_sql)
            .unwrap_or_else(|e| panic!("Failed to transpile {}: {}", version, e));

        all_warnings.extend(out.warnings.clone());

        let path = format!("{}/{}.sql", transpiled_dir, version);
        std::fs::write(&path, &out.sql)
            .unwrap_or_else(|e| panic!("Failed to write {}: {}", path, e));

        let c_lines: Vec<&str> = canonical_sql.lines().collect();
        let t_lines: Vec<&str> = out.sql.lines().collect();
        println!("║  ── {} ──────────────────────────────────────────────────────", version);

        for (i, &line) in c_lines.iter().enumerate() {
            let t_line = t_lines
                .get(i)
                .map(|s| format!(" → {}", s.trim()))
                .unwrap_or_default();
            let ln = if line.trim().starts_with("--") {
                format!("      {:<5}{}", line.trim(), t_line)
            } else {
                format!("{:>2}   {:<5}{}", i + 1, line.trim(), t_line)
            };
            println!("║  {}", ln);
        }
        println!();

        transpiled_sqls.push(out.sql);
    }

    if !all_warnings.is_empty() {
        println!("║  WARNINGS: {:?}", all_warnings);
    }
    println!("╠══════════════════════════════════════════════════════════════╣");

    let mut total_checks: usize = 0;
    let mut passed_checks: usize = 0;
    let mut apply_ok_all = true;

    for step_idx in 0..transpiled_sqls.len() {
        let pool = fresh_pool().await;

        for (i, sql) in transpiled_sqls.iter().enumerate().take(step_idx + 1) {
            let batch: String = sql
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string() + "\n")
                .collect();

            match sqlx::query(&batch).execute(&pool).await {
                Ok(r) => println!("║  [APPLY] V{}  rows_affected={} ✓", i + 1, r.rows_affected()),
                Err(e) => { println!("║  [FAIL]  V{}  ERROR: {}", i + 1, e); apply_ok_all = false; }
            }
        }

        // Expected columns for this step, keyed by table name
        let step_expected: Vec<(&str, &[(&str, &str)], &[(&str, &str)])> = match step_idx {
            0 => vec![("users",
                       &[("id","TEXT"),("username","TEXT"),("email","TEXT"),("created_at","TEXT"),("active","INTEGER")],
                       &[("id","TEXT"),("username","TEXT"),("email","TEXT"),("created_at","TEXT"),("active","INTEGER")])],
            1 => vec![("users",
                       &[("id","TEXT"),("username","TEXT"),("email","TEXT"),("created_at","TEXT"),("active","INTEGER"),("bio","TEXT"),("is_premium","INTEGER")],
                       &[("id","TEXT"),("username","TEXT"),("email","TEXT"),("created_at","TEXT"),("active","INTEGER"),("bio","TEXT"),("is_premium","INTEGER")])],
            2 => vec![
                ("users",
                 &[("id","TEXT"),("username","TEXT"),("email","TEXT"),("created_at","TEXT"),("active","INTEGER"),("bio","TEXT"),("is_premium","INTEGER")],
                 &[("id","TEXT"),("username","TEXT"),("email","TEXT"),("created_at","TEXT"),("active","INTEGER"),("bio","TEXT"),("is_premium","INTEGER")]),
                ("profiles",
                 &[("id","TEXT"),("user_id","TEXT"),("display_name","TEXT"),("avatar_url","TEXT"),("created_at","TEXT")],
                 &[("id","TEXT"),("user_id","TEXT"),("display_name","TEXT"),("avatar_url","TEXT"),("created_at","TEXT")]),
            ],
            _ => unreachable!(),
        };

        println!("║");
        println!("║  V{} — schema verification ({} tables)", step_idx + 1, step_expected.len());
        println!("║  ──────────────────────────────────────────────────────");

        for &(table, _users_exp, _tables_exp) in &step_expected {
            let exp = if table == "users" { _users_exp } else { _tables_exp };

            let rows = sqlx::query(&format!("PRAGMA table_info({})", table))
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

            for (cid_num, row) in rows.iter().enumerate() {
                let name:  String = row.try_get(1).unwrap_or_default();
                let rtype: String = row.try_get(2).unwrap_or_default();
                let pk:    i64    = row.try_get(5).unwrap_or(0);

                let exp_opt = exp.get(cid_num);
                let ok = exp_opt.map_or(false, |&(en, et)| {
                    en == name.as_str() && et == rtype.as_str()
                });
                let info = exp_opt
                    .map_or("<unexpected column>".to_string(), |&(en, et)| {
                        if ok { format!("ok: {} {}", en, et) } else { format!("expected: {} {}", en, et) }
                    });

                let mark = if ok { "✓" } else { "✗" };
                println!("║    {}  {:>2}  {:<18}  {:<12}  pk={}  {}", mark, cid_num, name, rtype, pk, info);
                total_checks += 1;
                if ok { passed_checks += 1; }
            }
        }
        println!("║  ──────────────────────────────────────────────────────");
        pool.close().await;
    }

    // ── Insert & read-back ─────────────────────────────────────────────────
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Insert & read-back");
    let pool = fresh_pool().await;

    for sql in &transpiled_sqls {
        let batch: String = sql
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string() + "\n")
            .collect();
        sqlx::query(&batch).execute(&pool).await.expect("replay migrations");
    }

    let uid = uuid::Uuid::new_v4();
    let pid = uuid::Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, username, email, active) VALUES (?1, ?2, ?3, 1)")
        .bind(uid.to_string()).bind("test_user").bind("test@example.com")
        .execute(&pool).await.expect("insert users");

    sqlx::query("INSERT INTO profiles (id, user_id, display_name) VALUES (?1, ?2, ?3)")
        .bind(pid.to_string()).bind(uid.to_string()).bind("Test Display")
        .execute(&pool).await.expect("insert profiles");

    let u = sqlx::query("SELECT id, username, email FROM users LIMIT 1")
        .fetch_one(&pool).await.expect("read users");
    let u_id:    String = u.try_get(0).unwrap_or_default();
    let u_name:  String = u.try_get(1).unwrap_or_default();
    let u_email: String = u.try_get(2).unwrap_or_default();
    println!("║  users    → {:<36} {}  {}", u_id, u_name, u_email);
    total_checks += 3;
    if u_name == "test_user" && u_email == "test@example.com" { passed_checks += 3; }

    let p = sqlx::query("SELECT id, user_id, display_name FROM profiles LIMIT 1")
        .fetch_one(&pool).await.expect("read profiles");
    let p_id:      String = p.try_get(0).unwrap_or_default();
    let p_user_id: String = p.try_get(1).unwrap_or_default();
    let p_display: String = p.try_get(2).unwrap_or_default();
    println!("║  profiles → {:<34} {} → {}", p_id, p_user_id, p_display);
    total_checks += 3;
    if p_user_id == uid.to_string() && p_display == "Test Display" { passed_checks += 3; }

    pool.close().await;

    // ── Summary ─────────────────────────────────────────────────────────────
    let pass     = total_checks == passed_checks;
    let mark     = if pass { " PASS " } else { " FAIL " };
    let final_md = if apply_ok_all && pass { " PASS " } else { " FAIL " };
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  {}  {:>3}/{:<3} checks passed  (warnings: {})",
        mark, passed_checks, total_checks, all_warnings.len()
    );
    println!(
        "║  {}  apply={}  schema={}",
        final_md,
        if apply_ok_all { "OK" } else { "ERR" },
        if pass { "OK" } else { "ERR" }
    );
    println!("╚══════════════════════════════════════════════════════════════╝");
}
