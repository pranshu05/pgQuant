use postgres::{Client, NoTls};
use rand::Rng;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    // Determine PostgreSQL version from env or default to 15
    let pg_version = std::env::var("PG_VERSION").unwrap_or_else(|_| "15".to_string());
    let port = 5432; // default port; adjust if needed

    // Start PostgreSQL using pgrx (background process)
    println!("Starting PostgreSQL {}...", pg_version);
    let mut pg_process = Command::new("cargo")
        .args(["pgrx", "start", &format!("pg{}", pg_version)])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start PostgreSQL");

    // Give it a moment to come up
    thread::sleep(Duration::from_secs(5));

    // Connect
    let mut client = loop {
        match Client::connect(
            &format!("host=localhost port={} user=postgres", port),
            NoTls,
        ) {
            Ok(c) => break c,
            Err(_) => {
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        }
    };

    // Load the extension
    client
        .batch_execute("CREATE EXTENSION IF NOT EXISTS pgquant;")
        .expect("Failed to create extension");

    // Create a temporary table with synthetic data
    client
        .batch_execute(
            "DROP TABLE IF EXISTS bench_prices;\
             CREATE TEMP TABLE bench_prices (symbol TEXT, date DATE, ret DOUBLE PRECISION);",
        )
        .expect("Failed to create temp table");

    // Insert many rows (e.g., 100k)
    let symbols = ["A", "B", "C", "D", "E"]; // 5 symbols
    let start_date = chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    let num_days = 20000; // rows = symbols * days = 100k
    let mut rng = rand::thread_rng();
    let insert_stmt = client
        .prepare("INSERT INTO bench_prices VALUES ($1, $2, $3)")
        .expect("Failed to prepare insert");
    let mut tx = client.transaction().expect("failed tx");
    for day in 0..num_days {
        let date = start_date + chrono::Duration::days(day as i64);
        for &sym in &symbols {
            let ret: f64 = rng.gen_range(-0.05..0.05);
            let date_str = date.to_string();
            tx.execute(&insert_stmt, &[&sym, &date_str, &ret])
                .expect("insert failed");
        }
    }
    tx.commit().expect("commit failed");
    println!("Inserted {} rows", symbols.len() * num_days);

    // Benchmark pgquant_sample_cov
    let query = "SELECT symbol, date, ret FROM bench_prices ORDER BY date, symbol";
    let sql = format!(
        "SELECT * FROM pgquant_sample_cov('{}')",
        query.replace('\'', "''")
    );

    let start = Instant::now();
    let rows = client.query(&*sql, &[]).expect("benchmark query failed");
    let duration = start.elapsed();
    println!(
        "pgquant_sample_cov returned {} rows in {:.2?}",
        rows.len(),
        duration
    );

    // Benchmark shrinkage
    let sql2 = format!(
        "SELECT * FROM pgquant_shrinkage_cov_lw('{}')",
        query.replace('\'', "''")
    );
    let start2 = Instant::now();
    let rows2 = client.query(&*sql2, &[]).expect("shrinkage query failed");
    let duration2 = start2.elapsed();
    println!(
        "pgquant_shrinkage_cov_lw returned {} rows in {:.2?}",
        rows2.len(),
        duration2
    );

    // Shut down PostgreSQL
    println!("Shutting down PostgreSQL...");
    let _ = pg_process.kill();
}
