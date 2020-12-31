#[macro_use]
extern crate lazy_static;

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
mod highlight;
mod io;
mod params;
use highlight::highlight;
use io::{get_paste, store_paste};
use params::IsPlaintextRequest;

use askama::{Html as AskamaHtml, MarkupDisplay, Template};
use auth::AuthKey;
use rocket::data::ToByteUnit;
use rocket::fairing::AdHoc;
use rocket::http::{ContentType, RawStr, Status};
use rocket::request::Form;
use rocket::response::content::{Content, Html};
use rocket::response::Redirect;
use rocket::Data;
use rocket::State;

use std::borrow::Cow;

use tokio::io::AsyncReadExt;

fn default_id_length() -> usize {
    4
}

#[derive(serde::Deserialize)]
struct BibinConfig {
    password: AuthKey,
    prefix: String,
    #[serde(default = "default_id_length")]
    id_length: usize,
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
) -> Result<Redirect, Status> {
    let form_data = input.into_inner();
    if !form_data.password.is_valid(&config.password) {
        Err(Status::Unauthorized)
    } else {
        match store_paste(config.id_length, form_data.val).await {
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

    match store_paste(config.id_length, data).await {
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

///
/// Show paste page
///

#[derive(Template)]
#[template(path = "paste.html")]
struct ShowPaste<'a> {
    content: MarkupDisplay<AskamaHtml, Cow<'a, String>>,
}

#[get("/<name>/qr")]
async fn get_qr(name: String, config: State<'_, BibinConfig>) -> Result<Content<Vec<u8>>, Status> {
    let mut splitter = name.splitn(2, '.');
    let key = splitter.next().ok_or(Status::NotFound)?;

    let _entry = &*get_paste(key).await.ok_or(Status::NotFound)?;

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
) -> Result<RedirectOrContent, Status> {
    let mut splitter = key.splitn(2, '.');
    let key = splitter.next().ok_or(Status::NotFound)?;
    let ext = splitter.next();

    let entry = &*get_paste(key).await.ok_or(Status::NotFound)?;

    if let Some(extension) = ext {
        match extension {
            "url" => return Ok(RedirectOrContent::Redirect(Redirect::to(entry.to_string()))),
            "qr" => match qrcode_generator::to_png_to_vec(entry, QrCodeEcc::Medium, 1024) {
                Ok(code) => return Ok(RedirectOrContent::Binary(Content(ContentType::PNG, code))),
                Err(e) => {
                    warn!("ERROR: when generating qr code: {}", e);
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
            entry.to_string(),
        )))
    } else {
        let code_highlighted = match ext {
            Some(extension) => match highlight(&entry, extension) {
                Some(html) => html,
                None => return Err(Status::NotFound),
            },
            None => String::from(RawStr::from_str(entry).html_escape()),
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
fn rocket() -> rocket::Rocket {
    rocket::ignite()
        .mount("/", routes![index, submit, submit_raw, show_paste, get_qr])
        .attach(AdHoc::config::<BibinConfig>())
}
