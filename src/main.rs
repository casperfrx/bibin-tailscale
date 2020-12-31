#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate rocket;

extern crate qrcode_generator;

use qrcode_generator::QrCodeEcc;

extern crate askama;

mod highlight;
mod io;
mod params;

use highlight::highlight;
use io::{get_paste, store_paste};
use params::IsPlaintextRequest;

use askama::{Html as AskamaHtml, MarkupDisplay, Template};

use rocket::fairing::AdHoc;
use rocket::http::{ContentType, RawStr, Status};
use rocket::request::Form;
use rocket::response::content::{Content, Html};
use rocket::response::Redirect;
use rocket::Data;
use rocket::State;

use std::borrow::Cow;

use tokio::io::AsyncReadExt;

struct Password(String);
struct Prefix(String);
struct IdLength(usize);

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
/// Submit Paste
///

#[derive(FromForm, Clone)]
struct IndexForm {
    val: String,
    password: String,
}

#[post("/", data = "<input>")]
async fn submit<'s>(
    state: State<'s, Password>,
    input: Form<IndexForm>,
    id_length: State<'_, IdLength>,
) -> Result<Redirect, Status> {
    let form_data = input.into_inner();
    if form_data.password != state.0 {
        Err(Status::Unauthorized)
    } else {
        match store_paste(id_length.0, form_data.val).await {
            Ok(id) => {
                let uri = uri!(show_paste: &id);
                Ok(Redirect::to(uri))
            }
            Err(e) => {
                println!("ERROR: {}", e);
                Err(Status::InternalServerError)
            }
        }
    }
}

#[put("/<password>", data = "<input>")]
async fn submit_raw(
    input: Data,
    state: State<'_, Password>,
    password: String,
    prefix: State<'_, Prefix>,
    id_length: State<'_, IdLength>,
) -> Result<String, Status> {
    if password != state.0 {
        return Err(Status::Unauthorized);
    }

    let mut data = String::new();
    input
        .open()
        .take(5 * 1024 * 1040) // Max size: 5MB
        .read_to_string(&mut data)
        .await
        .map_err(|_| Status::InternalServerError)?;

    match store_paste(id_length.0, data).await {
        Ok(id) => {
            let uri = uri!(show_paste: &id);
            Ok(format!("{}{}", prefix.0, uri))
        }
        Err(e) => {
            println!("ERROR: {}", e);
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
async fn get_qr(name: String, prefix: State<'_, Prefix>) -> Result<Content<Vec<u8>>, Status> {
    let mut splitter = name.splitn(2, '.');
    let key = splitter.next().ok_or_else(|| Status::NotFound)?;

    let _entry = &*get_paste(key).await.ok_or_else(|| Status::NotFound)?;

    let result =
        qrcode_generator::to_png_to_vec(format!("{}/{}", prefix.0, &name), QrCodeEcc::Low, 1024)
            .unwrap();
    Ok(Content(ContentType::PNG, result))
}

#[get("/<key>")]
async fn show_paste(key: String, plaintext: IsPlaintextRequest) -> Result<Content<String>, Status> {
    let mut splitter = key.splitn(2, '.');
    let key = splitter.next().ok_or_else(|| Status::NotFound)?;
    let ext = splitter.next();

    let entry = &*get_paste(key).await.ok_or_else(|| Status::NotFound)?;

    if *plaintext {
        Ok(Content(ContentType::Plain, entry.to_string()))
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
            Ok(html) => Ok(Content(ContentType::HTML, html)),
            Err(_) => Err(Status::InternalServerError),
        }
    }
}

fn main() {
    if let Err(error) = rocket::ignite()
        .attach(AdHoc::on_attach("Reading Config", |rck| {
            let mut error = false;

            let rck = match rck.config().get_int("idlength") {
                Err(_) => {
                    println!("idlength setting not provided, defaulting to 4");
                    rck.manage(IdLength(4))
                }
                Ok(v) => rck.manage(IdLength(v as usize)),
            };

            let rck = match rck.config().get_string("password") {
                Err(e) => {
                    println!(
                        "Error: {}.\nCannot read the password in the Rocket configuration",
                        e
                    );
                    error = true;
                    rck
                }
                Ok(v) => rck.manage(Password(v)),
            };

            let rck = match rck.config().get_string("prefix") {
                Err(e) => {
                    println!(
                        "Error: {}.\nCannot read the prefix in the Rocket configuration",
                        e
                    );
                    error = true;
                    rck
                }
                Ok(v) => rck.manage(Prefix(v)),
            };

            if error {
                Err(rck)
            } else {
                Ok(rck)
            }
        }))
        .mount("/", routes![index, submit, submit_raw, show_paste, get_qr])
        .launch()
    {
        println!("Error: {}", error);
    }
}
