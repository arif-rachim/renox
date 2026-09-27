//! Made with `rnx make:module documents` and `rnx make:model Document --module documents -m`.

use std::time::Duration;

use renox::prelude::*;
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
    }
}

async fn index(State(db): State<Db>) -> Result<View> {
    let documents = Document::query().latest().get(&db).await?;
    Ok(view("documents/index.html", context! { documents }))
}

#[derive(Deserialize)]
struct PhotoForm {
    title: String,
    photo: Upload,
}

impl Validate for PhotoForm {
    fn rules(&self, v: &mut Validator) {
        v.field("title", &self.title).required().max(100);
        // Checked by content, not by name: a text file called x.png is refused.
        v.field("photo", &self.photo).required().image().max(2048); // KB
    }
}

async fn store_photo(
    State(state): State<AppState>,
    Valid(form): Valid<PhotoForm>,
) -> Result<Redirect> {
    let key = form.photo.store_public(&state.storage, "photos").await?;
    save(&state.db, form.title, "photo", key, &form.photo).await?;
    Ok(Redirect::to("/"))
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
) -> Result<Redirect> {
    let key = form.invoice.store(&state.storage, "invoices").await?; // private
    save(&state.db, form.title, "invoice", key, &form.invoice).await?;
    Ok(Redirect::to("/"))
}

async fn save(db: &Db, title: String, kind: &str, file_key: String, upload: &Upload) -> Result {
    let document = Document {
        title,
        kind: kind.into(),
        file_key,
        file_name: upload.file_name.clone(),
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
