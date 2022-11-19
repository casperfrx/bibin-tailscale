#![allow(clippy::unnecessary_lazy_evaluations)]
/// Until https://github.com/rust-lang/rust-clippy/pull/9486
/// lands into clippy

#[macro_use]
extern crate rocket;

#[macro_use]
extern crate log;

extern crate base64;

extern crate qrcode_generator;

extern crate askama;

extern crate serde;

mod auth;
mod config;
mod get;
mod highlight;
mod io;
mod isplaintextrequest;

use auth::AuthKey;
use config::BibinConfig;
use highlight::Highlighter;
use io::{delete_paste, store_paste, store_paste_given_id};
use rocket::data::ToByteUnit;
use rocket::form::Form;
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::tokio::io::AsyncReadExt;
use rocket::uri;
use rocket::Data;
use rocket::State;

use io::{ReadPool, WritePool};

///
/// Homepage
///

#[derive(Responder)]
pub enum HtmlOrPlain {
    #[response(content_type = "html")]
    Html(String),

    #[response(content_type = "plain")]
    Plain(String),
}

#[allow(clippy::large_enum_variant)]
#[derive(Responder)]
pub enum RedirectOrContent {
    Redirect(Redirect),

    #[response(content_type = "image/png")]
    Png(Vec<u8>),

    #[response(content_type = "html")]
    Html(String),

    #[response(content_type = "plain")]
    Plain(String),
}

///
/// Submit Paste
///
#[derive(FromForm, Clone)]
struct IndexForm {
    val: String,
    password: AuthKey,
}

#[post("/", data = "<input>")]
async fn submit(
    config: &State<BibinConfig>,
    input: Form<IndexForm>,
    pool: &State<WritePool>,
) -> Result<Redirect, Status> {
    let form_data = input.into_inner();
    if !form_data.password.is_valid(&config.password) {
        Err(Status::Unauthorized)
    } else {
        match store_paste(pool, config.id_length, config.max_entries, form_data.val).await {
            Ok(id) => {
                let uri = uri!(get::show_paste(id));
                Ok(Redirect::to(uri))
            }
            Err(e) => {
                error!("[SUBMIT] {} (pool {:?})", e, pool.0);
                Err(Status::InternalServerError)
            }
        }
    }
}

#[post("/<key>", data = "<input>")]
async fn submit_with_key(
    config: &State<BibinConfig>,
    input: Form<IndexForm>,
    pool: &State<WritePool>,
    key: String,
) -> Result<Redirect, Status> {
    let form_data = input.into_inner();
    if !form_data.password.is_valid(&config.password) {
        Err(Status::Unauthorized)
    } else {
        match store_paste_given_id(pool, key, form_data.val).await {
            Ok(id) => {
                let uri = uri!(get::show_paste(id));
                Ok(Redirect::to(uri))
            }
            Err(e) => {
                error!("[SUBMIT_WITH_KEY] {} (pool {:?})", e, pool.0);
                Err(Status::InternalServerError)
            }
        }
    }
}

#[put("/", data = "<input>")]
async fn submit_raw(
    input: Data<'_>,
    config: &State<BibinConfig>,
    password: auth::AuthKey,
    pool: &State<WritePool>,
) -> Result<String, Status> {
    if !password.is_valid(&config.password) {
        return Err(Status::Unauthorized);
    }

    let mut data = String::new();
    input
        .open(5.megabytes())
        .read_to_string(&mut data)
        .await
        .map_err(|_| Status::InternalServerError)?;

    match store_paste(pool, config.id_length, config.max_entries, data).await {
        Ok(id) => {
            let uri = uri!(get::show_paste(id));
            Ok(format!("{}{}", config.prefix, uri))
        }
        Err(e) => {
            error!("[SUBMIT_RAW] {} (pool {:?})", e, pool.0);
            Err(Status::InternalServerError)
        }
    }
}

#[put("/<key>", data = "<input>")]
async fn submit_raw_with_key(
    input: Data<'_>,
    config: &State<BibinConfig>,
    password: auth::AuthKey,
    pool: &State<WritePool>,
    key: String,
) -> Result<String, Status> {
    if !password.is_valid(&config.password) {
        return Err(Status::Unauthorized);
    }

    let mut data = String::new();
    input
        .open(5.megabytes())
        .read_to_string(&mut data)
        .await
        .map_err(|_| Status::InternalServerError)?;

    match store_paste_given_id(pool, key, data).await {
        Ok(id) => {
            let uri = uri!(get::show_paste(id));
            Ok(format!("{}{}", config.prefix, uri))
        }
        Err(e) => {
            error!("[SUBMIT_RAW_WITH_KEY] {} (pool {:?})", e, pool.0);
            Err(Status::InternalServerError)
        }
    }
}

#[delete("/<id>")]
async fn delete(
    id: String,
    config: &State<BibinConfig>,
    password: auth::AuthKey,
    pool: &State<WritePool>,
) -> Result<String, Status> {
    if !password.is_valid(&config.password) {
        return Err(Status::Unauthorized);
    }

    match delete_paste(pool, id).await {
        Ok(id) => Ok(format!("{} deleted", id)),
        Err(e) => {
            error!("[DELETE_PASTE] {}", e);
            Err(Status::InternalServerError)
        }
    }
}

#[rocket::launch]
async fn rocket() -> rocket::Rocket<rocket::Build> {
    let highlighter = Highlighter::new();

    let rkt = rocket::Rocket::build();

    // I would like to use the ADHoc helpers instead, but I need to configure the database before
    // starting rocket. I prefer to not register Pools that are in a non-working state, and then
    // read the config and init them.
    // With the current system the pools are either created and working or don't exist.
    let config = match rkt.figment().extract::<BibinConfig>() {
        Err(e) => {
            rocket::config::pretty_print_error(e);
            panic!("Configuration error");
        }
        Ok(config) => config,
    };

    let write_pool = WritePool::new(&config.database_file)
        .await
        .expect("Error when creating the writing pool");

    write_pool
        .init()
        .await
        .expect("Error during initialization");

    let read_pool = ReadPool::new(&config.database_file, config.database_connections)
        .await
        .expect("Error when creating the reading pool");

    // 16 is the ID field size in the db
    if config.id_length > 16 {
        panic!("The maximum ID size is 16");
    }

    rkt.mount(
        "/",
        routes![
            get::index,
            submit,
            submit_with_key,
            submit_raw,
            submit_raw_with_key,
            get::show_paste,
            get::get_qr,
            get::get_raw,
            delete
        ],
    )
    .manage(config)
    .manage(highlighter)
    .manage(read_pool)
    .manage(write_pool)
}
