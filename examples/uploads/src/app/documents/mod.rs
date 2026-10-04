//! Made with `rnx make:module documents` and `rnx make:migration create_documents_table`.
//! The `Document` model is written here in the module (its table is
//! `documents`, named with `#[model(table = …)]`).

use std::time::Duration;

use renox::Download;
use renox::prelude::*;
use renox::validation::Dimensions;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "documents")]
pub struct Document {
    pub id: i64,
    pub title: String,
    /// `photo` (public) or `invoice` (private).
    pub kind: String,
    /// Where the file is in storage.
    pub file_key: String,
    /// The name it was uploaded with, for downloads.
    pub file_name: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

pub struct Documents;

impl Module for Documents {
    fn name(&self) -> &'static str {
        "documents"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", index)
            .name("home")
            .post("/photos", store_photo)
            .name("photos.store")
            .post("/invoices", store_invoice)
            .name("invoices.store")
            .get("/invoices/{id}/download", download)
            .name("invoices.download")
            .get("/invoices/{id}", view_invoice)
            .name("invoices.show")
            .delete("/documents/{id}", destroy)
            .name("documents.destroy")
    }
}

async fn index(State(db): State<Db>) -> Result<View> {
    let documents = Document::query().latest().get(&db).await?;
    Ok(view("documents/index.html", context! { documents }))
}

#[derive(Deserialize)]
struct PhotoForm {
    title: String,
    /// `<input type="file" name="photos" multiple>`: one or more files.
    #[serde(default)]
    photos: Vec<Upload>,
}

impl Validate for PhotoForm {
    fn rules(&self, v: &mut Validator) {
        v.field("title", &self.title).required().max(100);
        v.field("photos", &self.photos).required().max(10); // at most 10 files
        // Each file checked by content, not by name: a text file called x.png
        // is refused. Its size in pixels comes from the image's header.
        let size = Dimensions::new().max_width(6000).max_height(6000);
        v.each("photos", &self.photos, |photo| {
            photo.image().max(2048).dimensions(&size) // KB, then pixels
        });
    }
}

async fn store_photo(
    State(state): State<AppState>,
    htmx: Htmx,
    Valid(form): Valid<PhotoForm>,
) -> Result<(Toast, Response)> {
    let count = form.photos.len();
    for (i, photo) in form.photos.iter().enumerate() {
        let key = photo.store_public(&state.storage, "photos").await?;
        let title = if count > 1 {
            format!("{} ({}/{count})", form.title, i + 1)
        } else {
            form.title.clone()
        };
        save(&state.db, title, "photo", key, photo).await?;
    }
    let toast = if count == 1 {
        Toast::success("Photo uploaded.")
    } else {
        Toast::success(format!("{count} photos uploaded."))
    };
    // `HX-Redirect` for the htmx form (the browser loads the page anew), a
    // 303 otherwise. Either way the toast waits in the session for that page.
    Ok((toast, htmx.redirect(&state.url("home", &[])?)))
}

#[derive(Deserialize)]
struct InvoiceForm {
    title: String,
    invoice: Upload,
}

impl Validate for InvoiceForm {
    fn rules(&self, v: &mut Validator) {
        v.field("title", &self.title).required().max(100);
        v.field("invoice", &self.invoice)
            .required()
            .mimes(&["pdf"])
            .max(5120);
    }
}

async fn store_invoice(
    State(state): State<AppState>,
    Valid(form): Valid<InvoiceForm>,
) -> Result<(Toast, Redirect)> {
    let key = form.invoice.store(&state.storage, "invoices").await?; // private
    save(&state.db, form.title, "invoice", key, &form.invoice).await?;
    Ok((
        Toast::success("Invoice uploaded. It stays private."),
        Redirect::route("home", &[])?,
    ))
}

async fn save(db: &Db, title: String, kind: &str, file_key: String, upload: &Upload) -> Result {
    let document = Document {
        title,
        kind: kind.into(),
        file_key,
        file_name: upload.file_name().to_owned(),
        ..Default::default()
    };
    Document::create(db, document).await?;
    Ok(())
}

/// A private file: a signed link that works for five minutes (on S3, a
/// presigned URL). Check who may download before handing it out.
async fn download(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Redirect> {
    let document = Document::find_or_404(&state.db, id).await?;
    if document.kind != "invoice" {
        return Err(Error::NotFound);
    }
    let url = state
        .storage
        .temporary_url(&state, &document.file_key, Duration::from_secs(300))
        .await?;
    Ok(Redirect::to(&url))
}

/// The same private file, sent by the app itself with its original name
/// (the check above applies here too).
async fn view_invoice(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Download> {
    let document = Document::find_or_404(&state.db, id).await?;
    if document.kind != "invoice" {
        return Err(Error::NotFound);
    }
    Ok(
        Download::from_storage(&state.storage, &document.file_key, document.file_name)
            .await?
            .inline(),
    )
}

/// Deletes a document and its stored file, behind the kit's `confirm` sheet.
/// The row goes first: a file left behind by a failed delete is only wasted
/// space, while a row pointing at a missing file would be a broken link.
async fn destroy(State(state): State<AppState>, Path(id): Path<i64>) -> Result<(Toast, Redirect)> {
    let mut document = Document::find_or_404(&state.db, id).await?;
    document.delete(&state.db).await?;
    state.storage.delete(&document.file_key).await?;
    Ok((
        Toast::success(format!("“{}” deleted.", document.title)),
        Redirect::route("home", &[])?,
    ))
}
