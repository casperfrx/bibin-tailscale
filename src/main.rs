#[macro_use]
extern crate rocket;

#[macro_use]
extern crate log;

mod auth;
mod config;
mod get;
mod highlight;
mod io;
mod isplaintextrequest;
mod write;

use auth::AuthKey;
use config::BibinConfig;
use highlight::Highlighter;
use rocket::response::Redirect;

use io::{ReadPool, WritePool};

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

#[derive(FromForm, Clone)]
pub struct IndexForm {
    val: String,
    password: AuthKey,
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
            write::submit,
            write::submit_with_key,
            write::submit_raw,
            write::submit_raw_with_key,
            get::get_item,
            get::get_qr,
            get::all_entries,
            get::get_item_raw,
            write::delete
        ],
    )
    .manage(config)
    .manage(highlighter)
    .manage(read_pool)
    .manage(write_pool)
}
