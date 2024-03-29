use actix_web::http::StatusCode;
use actix_web::{get, middleware, web, App, HttpResponse, HttpServer};
use dotenv::dotenv;
use sqlx::{sqlite::SqliteConnectOptions, Error, SqlitePool};
use std::path::Path;
use rand::Rng;

#[allow(dead_code)]
struct CopyPasta {
    id: i64,
    body: String
}

struct MaxId {
    id: Option<i32>
}

struct AppState {
    db: SqlitePool,
    res_code: u16
}

async fn connect(filename: impl AsRef<Path>) -> Result<SqlitePool, Error> {
    let options = SqliteConnectOptions::new()
        .filename(filename)
        .create_if_missing(false);

    SqlitePool::connect_with(options).await
}

async fn get_db_size(data: &web::Data<AppState>) -> i64 {
    match sqlx::query_as!(
        MaxId,
        r#"SELECT max(id) AS id FROM copypastas;"#,
    )
    .fetch_one(&data.db)
    .await {
        Ok(o) => o.id.unwrap_or(388800) as i64,
        Err(_) => 388800,
    }
}

async fn gen_copypasta(data: &web::Data<AppState>) -> Option<CopyPasta> {
    let mut rng = rand::thread_rng();
    let num: i64 = rng.gen_range(0..get_db_size(&data).await);
    match sqlx::query_as!(
        CopyPasta,
        r#"SELECT * FROM copypastas WHERE id = ?"#,
        num
    )
    .fetch_one(&data.db)
    .await {
        Ok(o) => Some(o),
        Err(_) => None
    }
}

#[get("/{tail:.*}")]
async fn default(data: web::Data<AppState>) -> HttpResponse {
    let mut copypasta: Option<CopyPasta> = gen_copypasta(&data).await;
    while copypasta.is_none() {
        copypasta = gen_copypasta(&data).await;
    }
        HttpResponse::build(StatusCode::from_u16(data.res_code).unwrap()) // Safety: valid status code
        .content_type("text/html")
        .body(format!("<!DOCTYPE html><html><head><meta charset=\"UTF-8\"></head><body>{}</body></html>", copypasta.unwrap().body)) // Safety: while loop blocks None type, safe to unwrap.
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    match dotenv() {
        Ok(_) => (),
        Err(_) => {
            eprintln!("No env file found, continuing.");
        }
    }

    let response_code = match std::env::var("RESPONSE_CODE") {
        Ok(rescode) => rescode.parse::<u16>().unwrap_or(418),
        Err(_) => {
            println!("RESPONSE_CODE env variable not set, using default \"418\"");
            std::env::set_var("RESPONSE_CODE", "418");
            418
        }
    };

    let database_path = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            println!(
                "DATABASE_URL env variable not set, using default \"./data/copypastas.sqlite\"."
            );
            std::env::set_var("DATABASE_URL", "./data/copypastas.sqlite");
            "./data/copypastas.sqlite".to_string()
        }
    };

    if !Path::new(&database_path).exists() {
        eprintln!(
            "Database file \"{database_path}\" does not exist. Please create it and try again."
        );
        std::process::exit(1);
    }

    let raw_port = match std::env::var("TEAPOT_FORTUNE_PORT") {
        Ok(port) => port,
        Err(_) => {
            println!(
                "TEAPOT_FORTUNE_PORT env variable not set, using default of 6757."
            );
            std::env::set_var("PORT", "6757");
            "6757".to_string()
        }
    };

    let port = match raw_port.parse::<u16>() {
        Ok(port) => port,
        Err(_) => {
            eprintln!("TEAPOT_FORTUNE_PORT \"{raw_port}\" is not a valid port number. Using default of 6757");
            std::env::set_var("PORT", "6757");
            6757
        }
    };

    let conn = connect(database_path)
        .await
        .expect("Failed to open sqlite database.");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState { db: conn.clone(), res_code: response_code}))
            .wrap(middleware::Compress::default())
            .wrap(middleware::DefaultHeaders::new().add(("CDN-Cache-Control", "no-store")).add(("Cache-Control", "no-store")))
            .service(default)
    })
    .bind(("0.0.0.0", port))?
    .workers(5)
    .run()
    .await
}
