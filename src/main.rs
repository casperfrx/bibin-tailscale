#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate rocket;

extern crate askama;

mod highlight;
mod io;
mod params;

use highlight::highlight;
use io::{generate_id, get_paste, store_paste};
use params::{HostHeader, IsPlaintextRequest};

use askama::{Html as AskamaHtml, MarkupDisplay, Template};

use rocket::http::{ContentType, RawStr, Status};
use rocket::request::Form;
use rocket::response::content::{Content, Html};
use rocket::response::Redirect;
use rocket::Data;
use rocket::fairing::AdHoc;
use rocket::State;

use std::borrow::Cow;

use tokio::io::AsyncReadExt;


struct Password(String);


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
    password: String
}

#[post("/", data = "<input>")]
async fn submit<'s>(state: State<'s, Password>, input: Form<IndexForm>) -> Result<Redirect, Status> {
    let id = generate_id();
    let uri = uri!(show_paste: &id);
    let form_data = input.into_inner();
    return if form_data.password != state.0 {
        Err(Status::Unauthorized)
    } else {
        store_paste(id, form_data.val).await;
        Ok(Redirect::to(uri))
    }
}

#[put("/<password>", data = "<input>")]
async fn submit_raw<'s>(input: Data, state: State<'s, Password>, password:String, host: HostHeader<'_>) -> Result<String, Status> {
    if password != state.0 {
        return Err(Status::Unauthorized);
    }

    let mut data = String::new();
    input.open().take(1024 * 1000)
        .read_to_string(&mut data).await
        .map_err(|_| Status::InternalServerError)?;

    let id = generate_id();
    let uri = uri!(show_paste: &id);

    store_paste(id, data).await;

    match *host {
        Some(host) => Ok(format!("https://{}{}", host, uri)),
        None => Ok(format!("{}", uri)),
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
        .attach(AdHoc::on_attach("Password Config", |rck| {
            println!("Adding password from config...");
            let password = rck.config()
                .get_string("password");
            match password {
                Err(e) =>  {
                    println!("Error: {}.\nCannot read a password in the Rocket configuration", e);
                    Err(rck)
                }
                Ok(password) => {
                    Ok(rck.manage(Password(password)))
                }
            }
        })
        )
        .mount("/", routes![index, submit, submit_raw, show_paste])
        .launch() {
            println!("Error: {}", error);
        }
}
