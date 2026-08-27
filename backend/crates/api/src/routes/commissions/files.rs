//! `POST /commissions/{id}/files` and `GET /commissions/{id}/files/{file_id}` —
//! a Participant uploads a work-in-progress file entry, and a Participant
//! retrieves one. Uploading never mutates any status.

use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{
        ChangelogEntryKind, CommissionFile, CommissionId, FileKey, FileMetadata, FileName,
        NewChangelogEntry,
    },
    ports::UnitOfWork,
};
use serde::Serialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// `POST /commissions/{id}/files`'s `201` body: the uploaded entry's key — see
/// [`upload_file`].
#[derive(Serialize)]
struct UploadFileResponse {
    id: Uuid,
}

/// Uploads a file entry as multipart form data. Any-Participant-gated;
/// `413` if oversize, `422` for a malformed body or bad filename. Returns
/// `201 Created` with `{ "id": "<uuid>" }`.
pub(super) async fn upload_file(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    multipart: Multipart,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let upload = read_file_part(multipart).await?;
    let max = state.config.max_upload_bytes;
    if upload.bytes.len() as u64 > max {
        return Err(Problem::payload_too_large(format!(
            "The file exceeds the {max}-byte upload limit."
        )));
    }
    if upload.bytes.is_empty() {
        return Err(Problem::invalid_request("The uploaded file is empty."));
    }

    let filename = FileName::try_new(upload.filename.unwrap_or_default())
        .map_err(|e| Problem::invalid_request(format!("Invalid filename: {e}.")))?;
    let metadata = FileMetadata::new(
        filename,
        upload.content_type.unwrap_or_default(),
        upload.bytes.len() as i64,
    );

    let key = FileKey::generate();
    // Precedes the transaction: bytes can't ride a unit of work.
    state.files.put(key, &metadata, &upload.bytes).await?;

    let now = Utc::now();
    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::FileAdded,
        user.id,
        json!({
            "file_id": *key,
            "filename": metadata.filename.as_str(),
            "content_type": metadata.content_type,
            "byte_size": metadata.byte_size,
        }),
        now,
    );
    let file = CommissionFile {
        id: key,
        commission_id: commission,
        uploaded_by: user.id,
        created_at: now,
    };
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().add_file(&file).await?;
            uow.changelog().append(&entry).await
        })
        .await?;

    let body = UploadFileResponse { id: *key };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

/// Retrieves a file entry's bytes. Any-Participant-gated; `404
/// file_not_found` for a key not in this commission. Always serves
/// `Content-Disposition: attachment` and `X-Content-Type-Options: nosniff` so
/// a stored SVG/HTML can never execute in the app origin.
pub(super) async fn download_file(
    State(state): State<AppState>,
    Path((id, file_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let key = FileKey::new(file_id);
    state
        .commissions
        .find_file(commission, key)
        .await?
        .ok_or_else(Problem::file_not_found)?;

    let stored = state
        .files
        .get(key)
        .await?
        .ok_or_else(|| Problem::internal_error("The file's contents are unavailable."))?;

    let content_type = HeaderValue::from_str(&stored.metadata.content_type)
        .unwrap_or_else(|_| HeaderValue::from_static(FileMetadata::DEFAULT_CONTENT_TYPE));
    let disposition =
        HeaderValue::from_str(&content_disposition(stored.metadata.filename.as_str()))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment"));

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, disposition),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        Body::from(stored.bytes),
    )
        .into_response())
}

/// The `file` part of a multipart upload: its declared filename and content type
/// (both optional at the wire level) and its bytes.
struct UploadPart {
    filename: Option<String>,
    content_type: Option<String>,
    bytes: Vec<u8>,
}

/// Pulls the `file` part out of a `multipart/form-data` body. `422` if the
/// body isn't multipart, is malformed, or carries no `file` part.
async fn read_file_part(mut multipart: Multipart) -> Result<UploadPart, Problem> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| Problem::invalid_request("Malformed multipart body."))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().map(str::to_owned);
        let content_type = field.content_type().map(str::to_owned);
        let bytes = field
            .bytes()
            .await
            .map_err(|_| Problem::invalid_request("Could not read the uploaded file."))?
            .to_vec();
        return Ok(UploadPart {
            filename,
            content_type,
            bytes,
        });
    }
    Err(Problem::invalid_request(
        "Expected a 'file' part in the multipart body.",
    ))
}

/// Builds a `Content-Disposition: attachment` header value carrying the
/// filename safely across the ASCII-only header boundary (RFC 6266 + 5987).
fn content_disposition(filename: &str) -> String {
    let fallback: String = filename
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() && c != '"' && c != '\\' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{}",
        rfc5987_encode(filename)
    )
}

/// Percent-encodes `s` per RFC 5987's `attr-char` set; everything else
/// becomes `%XX`.
fn rfc5987_encode(s: &str) -> String {
    const ATTR_CHAR_EXTRA: &[u8] = b"!#$&+-.^_`|~";
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || ATTR_CHAR_EXTRA.contains(&b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_file_response_serializes_to_a_bare_id_object() {
        let id = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000006").unwrap();
        let body = UploadFileResponse { id };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            format!("{{\"id\":\"{id}\"}}")
        );
    }

    #[test]
    fn content_disposition_is_attachment_and_encodes_safely() {
        let value = content_disposition("réf sheet.png");
        assert!(
            value.starts_with("attachment; "),
            "always attachment: {value}"
        );
        assert!(
            value.contains("filename*=UTF-8''r%C3%A9f%20sheet.png"),
            "{value}"
        );
        assert!(value.contains("filename=\"r_f sheet.png\""), "{value}");
    }

    #[test]
    fn rfc5987_leaves_attr_chars_and_escapes_the_rest() {
        assert_eq!(rfc5987_encode("a-b_c.png"), "a-b_c.png");
        assert_eq!(rfc5987_encode("a b"), "a%20b");
        assert_eq!(rfc5987_encode("\""), "%22");
    }
}
