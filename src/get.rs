use crate::config::BibinConfig;
use crate::highlight::Highlighter;
use crate::io::{get_paste, ReadPool};
use crate::RedirectOrContent;
use crate::{isplaintextrequest::IsPlaintextRequest, HtmlOrPlain};
use qrcode_generator::QrCodeEcc;
use rocket::http::{RawStr, Status};
use rocket::response::Redirect;
use rocket::State;
use std::borrow::Cow;

use askama::{Html as AskamaHtml, MarkupDisplay, Template};

///
/// Show paste page
///

#[derive(Responder)]
#[response(content_type = "image/png")]
pub struct PngResponder(Vec<u8>);

#[derive(Template)]
#[template(path = "paste.html")]
struct ShowPaste<'a> {
    content: MarkupDisplay<AskamaHtml, Cow<'a, String>>,
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct Index;

#[derive(Template)]
#[template(path = "curl_help.txt")]
pub struct CurlIndex {
    root_url: String,
}

#[get("/")]
pub fn index(
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

#[get("/<name>/qr")]
pub async fn get_qr(
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

#[get("/<name>/raw")]
pub async fn get_raw(name: String, pool: &State<ReadPool>) -> Result<HtmlOrPlain, Status> {
    let mut splitter = name.splitn(2, '.');
    let key = splitter.next().ok_or(Status::NotFound)?;
    let content = match get_paste(pool, key).await {
        // TODO: not found or Internal error
        Ok(None) => return Err(Status::NotFound),
        Err(e) => {
            warn!("[GET_RAW] Error in get_paste: {}", e);
            return Err(Status::InternalServerError);
        }
        Ok(Some(content)) => content,
    };

    Ok(HtmlOrPlain::Plain(content))
}

#[get("/<key>")]
pub async fn show_paste(
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
                Ok(html) => html,
                Err(error) => {
                    error!("Error highlighting from extension {} {}", extension, error);
                    return Err(Status::InternalServerError);
                }
            },
            None => String::from(RawStr::new(&entry).html_escape()),
        };

        // Add <code> tags to enable line numbering with CSS
        let html = format!(
            "<code>{}</code>",
            code_highlighted.replace('\n', "</code><code>")
        );

        let content = MarkupDisplay::new_safe(Cow::Borrowed(&html), AskamaHtml);

        let template = ShowPaste { content };
        match template.render() {
            Ok(html) => Ok(RedirectOrContent::Html(html)),
            Err(_) => Err(Status::InternalServerError),
        }
    }
}
