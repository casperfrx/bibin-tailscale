#[macro_use]
extern crate rocket;

#[macro_use]
extern crate log;

extern crate base64;

extern crate qrcode_generator;

use qrcode_generator::QrCodeEcc;

extern crate askama;

extern crate serde;

extern crate tokio_compat_02;

mod auth;
mod highlight;
mod io;
mod params;
use highlight::Highlighter;
use io::{delete_paste, get_paste, store_paste, store_paste_given_id};
use params::IsPlaintextRequest;

use askama::{Html as AskamaHtml, MarkupDisplay, Template};
use auth::AuthKey;
use rocket::data::ToByteUnit;
use rocket::http::{ContentType, RawStr, Status};
use rocket::request::Form;
use rocket::response::content::{Content, Html};
use rocket::response::Redirect;
use rocket::Data;
use rocket::State;

use rocket::tokio::io::AsyncReadExt;

use tokio_compat_02::FutureExt;

use std::borrow::Cow;

use io::{ReadPool, WritePool};

fn default_id_length() -> usize {
    4
}

fn default_database_connections() -> u32 {
    10
}

fn default_max_entries() -> i32 {
    10000
}

fn default_database_file() -> String {
    ":memory:".to_owned()
}

#[derive(serde::Deserialize)]
struct BibinConfig {
    password: AuthKey,
    prefix: String,
    #[serde(default = "default_id_length")]
    id_length: usize,
    #[serde(default = "default_database_file")]
    database_file: String,
    #[serde(default = "default_database_connections")]
    database_connections: u32,
    #[serde(default = "default_max_entries")]
    max_entries: i32,
}

///
/// Homepage
///

#[derive(Template)]
#[template(path = "index.html")]
struct Index;

#[get("/")]
fn index() -> Result<Html<String>, Status> {
    Index
        .render()
        .map(Html)
        .map_err(|_| Status::InternalServerError)
}

///
/// This type allow us to either return a Content or a Redirect to another page
///

#[derive(Responder)]
enum RedirectOrContent {
    Redirect(Redirect),
    String(Content<String>),
    Binary(Content<Vec<u8>>),
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
async fn submit<'s>(
    config: State<'s, BibinConfig>,
    input: Form<IndexForm>,
    pool: State<'_, WritePool>,
) -> Result<Redirect, Status> {
    let form_data = input.into_inner();
    if !form_data.password.is_valid(&config.password) {
        Err(Status::Unauthorized)
    } else {
        match store_paste(&pool, config.id_length, config.max_entries, form_data.val).await {
            Ok(id) => {
                let uri = uri!(show_paste: &id);
                Ok(Redirect::to(uri))
            }
            Err(e) => {
                error!("[SUBMIT] {}", e);
                Err(Status::InternalServerError)
            }
        }
    }
}

#[post("/<key>", data = "<input>")]
async fn submit_with_key<'s>(
    config: State<'s, BibinConfig>,
    input: Form<IndexForm>,
    pool: State<'_, WritePool>,
    key: String,
) -> Result<Redirect, Status> {
    let form_data = input.into_inner();
    if !form_data.password.is_valid(&config.password) {
        Err(Status::Unauthorized)
    } else {
        match store_paste_given_id(&pool, key, form_data.val).await {
            Ok(id) => {
                let uri = uri!(show_paste: &id);
                Ok(Redirect::to(uri))
            }
            Err(e) => {
                error!("[SUBMIT] {}", e);
                Err(Status::InternalServerError)
            }
        }
    }
}

#[put("/", data = "<input>")]
async fn submit_raw(
    input: Data,
    config: State<'_, BibinConfig>,
    password: auth::AuthKey,
    pool: State<'_, WritePool>,
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

    match store_paste(&pool, config.id_length, config.max_entries, data).await {
        Ok(id) => {
            let uri = uri!(show_paste: &id);
            Ok(format!("{}{}", config.prefix, uri))
        }
        Err(e) => {
            error!("[SUBMIT_RAW] {}", e);
            Err(Status::InternalServerError)
        }
    }
}

#[put("/<key>", data = "<input>")]
async fn submit_raw_with_key(
    input: Data,
    config: State<'_, BibinConfig>,
    password: auth::AuthKey,
    pool: State<'_, WritePool>,
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

    match store_paste_given_id(&pool, key, data).await {
        Ok(id) => {
            let uri = uri!(show_paste: &id);
            Ok(format!("{}{}", config.prefix, uri))
        }
        Err(e) => {
            error!("[SUBMIT_RAW] {}", e);
            Err(Status::InternalServerError)
        }
    }
}

#[delete("/<id>")]
async fn delete(
    id: String,
    config: State<'_, BibinConfig>,
    password: auth::AuthKey,
    pool: State<'_, WritePool>,
) -> Result<String, Status> {
    if !password.is_valid(&config.password) {
        return Err(Status::Unauthorized);
    }

    match delete_paste(&pool, id).await {
        Ok(id) => Ok(format!("{} deleted", id)),
        Err(e) => {
            error!("[DELETE_PASTE] {}", e);
            Err(Status::InternalServerError)
        }
    }
}

///
/// Show paste page
///

#[derive(Template)]
#[template(path = "paste.html")]
struct ShowPaste<'a> {
    content: MarkupDisplay<AskamaHtml, Cow<'a, String>>,
}

#[get("/<name>/qr")]
async fn get_qr(
    name: String,
    config: State<'_, BibinConfig>,
    pool: State<'_, ReadPool>,
) -> Result<Content<Vec<u8>>, Status> {
    let mut splitter = name.splitn(2, '.');
    let key = splitter.next().ok_or(Status::NotFound)?;
    match get_paste(&pool, key).await {
        // TODO: not found or Internal error
        Ok(None) => return Err(Status::NotFound),
        Err(e) => {
            warn!("[GET_QR] Error in get_paste: {}", e);
            return Err(Status::InternalServerError);
        }
        Ok(Some(_)) => (),
    };

    let result = qrcode_generator::to_png_to_vec(
        format!("{}/{}", config.prefix, &name),
        QrCodeEcc::Medium,
        1024,
    )
    .unwrap();
    Ok(Content(ContentType::PNG, result))
}

#[get("/<key>")]
async fn show_paste(
    key: String,
    plaintext: IsPlaintextRequest,
    pool: State<'_, ReadPool>,
    highlighter: State<'_, Highlighter>,
) -> Result<RedirectOrContent, Status> {
    let mut splitter = key.splitn(2, '.');
    let key = splitter.next().ok_or(Status::NotFound)?;
    let ext = splitter.next();

    let entry = match get_paste(&pool, key).await {
        Ok(Some(data)) => data,
        Ok(None) => return Err(Status::NotFound),
        Err(e) => {
            warn!("[SHOW_PASTE] get_paste error: {}", e);
            return Err(Status::InternalServerError);
        }
    };

    if let Some(extension) = ext {
        match extension {
            "url" => return Ok(RedirectOrContent::Redirect(Redirect::to(entry))),
            "qr" => match qrcode_generator::to_png_to_vec(entry, QrCodeEcc::Medium, 1024) {
                Ok(code) => return Ok(RedirectOrContent::Binary(Content(ContentType::PNG, code))),
                Err(e) => {
                    warn!("[SHOW_PASTE] qrcode_generator: {}", e);
                    return Err(Status::InternalServerError);
                }
            },
            "b64" => {
                return Ok(RedirectOrContent::String(Content(
                    ContentType::Plain,
                    base64::encode(entry),
                )))
            }
            _ => (),
        }
    }

    if *plaintext {
        Ok(RedirectOrContent::String(Content(
            ContentType::Plain,
            entry,
        )))
    } else {
        let code_highlighted = match ext {
            Some(extension) => match highlighter.highlight(&entry, extension) {
                Some(html) => html,
                None => return Err(Status::NotFound),
            },
            None => String::from(RawStr::from_str(&entry).html_escape()),
        };

        // Add <code> tags to enable line numbering with CSS
        let html = format!(
            "<code>{}</code>",
            code_highlighted.replace("\n", "</code><code>")
        );

        let content = MarkupDisplay::new_safe(Cow::Borrowed(&html), AskamaHtml);

        let template = ShowPaste { content };
        match template.render() {
            Ok(html) => Ok(RedirectOrContent::String(Content(ContentType::HTML, html))),
            Err(_) => Err(Status::InternalServerError),
        }
    }
}

#[rocket::launch]
async fn rocket() -> rocket::Rocket {
    let highlighter = Highlighter::new();

    let rkt = rocket::ignite();

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
        .compat()
        .await
        .expect("Error when creating the writing pool");

    write_pool
        .init()
        .compat()
        .await
        .expect("Error during initialization");

    let read_pool = ReadPool::new(&config.database_file, config.database_connections)
        .compat()
        .await
        .expect("Error when creating the reading pool");

    // 16 is the ID field size in the db
    if config.id_length > 16 {
        panic!("The maximum ID size is 16");
    }

    rkt.mount(
        "/",
        routes![
            index,
            submit,
            submit_with_key,
            submit_raw,
            submit_raw_with_key,
            show_paste,
            get_qr,
            delete
        ],
    )
    .manage(config)
    .manage(highlighter)
    .manage(read_pool)
    .manage(write_pool)
}
