#[macro_use]
extern crate rocket;

#[macro_use]
extern crate log;

extern crate base64;

extern crate qrcode_generator;

use qrcode_generator::QrCodeEcc;

extern crate askama;

extern crate serde;

mod auth;
mod config;
mod highlight;
mod io;
mod isplaintextrequest;

use askama::{Html as AskamaHtml, MarkupDisplay, Template};
use auth::AuthKey;
use config::BibinConfig;
use highlight::Highlighter;
use io::{delete_paste, get_paste, store_paste, store_paste_given_id};
use isplaintextrequest::IsPlaintextRequest;
use rocket::data::ToByteUnit;
use rocket::form::Form;
use rocket::http::{RawStr, Status};
use rocket::response::Redirect;
use rocket::tokio::io::AsyncReadExt;
use rocket::uri;
use rocket::Data;
use rocket::State;

use std::borrow::Cow;

use io::{ReadPool, WritePool};

///
/// Homepage
///

#[derive(Template)]
#[template(path = "index.html")]
struct Index;

#[derive(Template)]
#[template(path = "curl_help.txt")]
struct CurlIndex {
    root_url: String,
}

#[derive(Responder)]
#[response(content_type = "image/png")]
struct PngResponder(Vec<u8>);


#[derive(Responder)]
enum HtmlOrPlain {
    #[response(content_type = "html")]
    Html(String),

    #[response(content_type = "plain")]
    Plain(String),
}

#[derive(Responder)]
enum RedirectOrContent {
    Redirect(Redirect),

    #[response(content_type = "image/png")]
    Png(Vec<u8>),

    #[response(content_type = "html")]
    Html(String),

    #[response(content_type = "plain")]
    Plain(String),
}

#[get("/")]
fn index(
    config: &State<BibinConfig>,
    plaintext: IsPlaintextRequest,
) -> Result<HtmlOrPlain, Status> {
    if plaintext.0 {
        CurlIndex {
            root_url: config.prefix.clone(),
        }
        .render()
        .map(HtmlOrPlain::Plain)
        .map_err(|_| Status::InternalServerError)
    } else {
        Index
            .render()
            .map(HtmlOrPlain::Html)
            .map_err(|_| Status::InternalServerError)
    }
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
                let uri = uri!(show_paste(id));
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
                let uri = uri!(show_paste(id));
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
            let uri = uri!(show_paste(id));
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
            let uri = uri!(show_paste(id));
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
    config: &State<BibinConfig>,
    pool: &State<ReadPool>,
) -> Result<PngResponder, Status> {
    let mut splitter = name.splitn(2, '.');
    let key = splitter.next().ok_or(Status::NotFound)?;
    match get_paste(pool, key).await {
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
    Ok(PngResponder(result))
}

#[get("/<key>")]
async fn show_paste(
    key: String,
    plaintext: IsPlaintextRequest,
    pool: &State<ReadPool>,
    highlighter: &State<Highlighter>,
) -> Result<RedirectOrContent, Status> {
    let mut splitter = key.splitn(2, '.');
    let key = splitter.next().ok_or(Status::NotFound)?;
    let ext = splitter.next();

    let entry = match get_paste(pool, key).await {
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
                Ok(code) => return Ok(RedirectOrContent::Png(code)),
                Err(e) => {
                    warn!("[SHOW_PASTE] qrcode_generator: {}", e);
                    return Err(Status::InternalServerError);
                }
            },
            "b64" => return Ok(RedirectOrContent::Plain(base64::encode(entry))),
            _ => (),
        }
    }

    if *plaintext {
        Ok(RedirectOrContent::Plain(entry))
    } else {
        let code_highlighted = match ext {
            Some(extension) => match highlighter.highlight(&entry, extension) {
                Some(html) => html,
                None => return Err(Status::NotFound),
            },
            None => String::from(RawStr::new(&entry).html_escape()),
        };

        // Add <code> tags to enable line numbering with CSS
        let html = format!(
            "<code>{}</code>",
            code_highlighted.replace("\n", "</code><code>")
        );

        let content = MarkupDisplay::new_safe(Cow::Borrowed(&html), AskamaHtml);

        let template = ShowPaste { content };
        match template.render() {
            Ok(html) => Ok(RedirectOrContent::Html(html)),
            Err(_) => Err(Status::InternalServerError),
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
