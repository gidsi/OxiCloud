use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use quick_xml::se::to_string;
use serde::Serialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

#[derive(sqlx::FromRow)]
pub struct FsNode {
    pub id: Uuid,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<i64>,
    pub mime_type: Option<String>,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename = "D:multistatus")]
pub struct MultiStatus {
    #[serde(rename = "@xmlns:D")]
    pub xmlns: String,
    #[serde(rename = "D:response")]
    pub responses: Vec<DavResponse>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct DavResponse {
    #[serde(rename = "D:href")]
    pub href: String,
    #[serde(rename = "D:propstat")]
    pub propstat: PropStat,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct PropStat {
    #[serde(rename = "D:prop")]
    pub prop: Prop,
    #[serde(rename = "D:status")]
    pub status: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Prop {
    #[serde(rename = "D:getcontenttype", skip_serializing_if = "Option::is_none")]
    pub getcontenttype: Option<String>,
    #[serde(rename = "D:getcontentlength", skip_serializing_if = "Option::is_none")]
    pub getcontentlength: Option<i64>,
    #[serde(rename = "D:resourcetype")]
    pub resourcetype: ResourceType,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ResourceType {
    #[serde(rename = "D:collection", skip_serializing_if = "Option::is_none")]
    pub collection: Option<()>,
}

pub async fn propfind(
    State(state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let depth = headers
        .get("Depth")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("infinity");

    let target_node = sqlx::query_as::<_, FsNode>(
        "SELECT id, path, is_dir, size, mime_type, parent_id FROM fs_nodes WHERE path = $1",
    )
    .bind(&path)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::DatabaseError)?;

    let Some(node) = target_node else {
        return Err(AppError::NotFound);
    };

    let mut responses = vec![build_dav_response(&node)];

    if depth == "1" && node.is_dir {
        let children = sqlx::query_as::<_, FsNode>(
            "SELECT id, path, is_dir, size, mime_type, parent_id FROM fs_nodes WHERE parent_id = $1",
        )
        .bind(node.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::DatabaseError)?;

        responses.extend(children.iter().map(build_dav_response));
    }

    let multistatus = MultiStatus {
        xmlns: "DAV:".to_string(),
        responses,
    };

    let xml = to_string(&multistatus).map_err(|_| AppError::SerializationError)?;
    let body = format!(r#"<?xml version="1.0" encoding="utf-8"?>{xml}"#);

    Ok((
        StatusCode::MULTI_STATUS,
        [("Content-Type", "application/xml; charset=utf-8")],
        body,
    )
        .into_response())
}

fn build_dav_response(node: &FsNode) -> DavResponse {
    let resourcetype = if node.is_dir {
        ResourceType {
            collection: Some(()),
        }
    } else {
        ResourceType { collection: None }
    };

    DavResponse {
        href: format!("/remote.php/webdav/{}", node.path),
        propstat: PropStat {
            prop: Prop {
                getcontenttype: node.mime_type.clone(),
                getcontentlength: node.size,
                resourcetype,
            },
            status: "HTTP/1.1 200 OK".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dav_response_xml_serialization() {
        let node = FsNode {
            id: Uuid::new_v4(),
            path: "test.txt".to_string(),
            is_dir: false,
            size: Some(1024),
            mime_type: Some("text/plain".to_string()),
            parent_id: None,
        };

        let response = build_dav_response(&node);
        let multistatus = MultiStatus {
            xmlns: "DAV:".to_string(),
            responses: vec![response],
        };

        let xml = to_string(&multistatus).expect("failed to serialize multistatus");

        assert!(xml.contains(r#"xmlns:D="DAV:""#));
        assert!(xml.contains("<D:href>/remote.php/webdav/test.txt</D:href>"));
        assert!(xml.contains("<D:getcontentlength>1024</D:getcontentlength>"));
        assert!(xml.contains("<D:getcontenttype>text/plain</D:getcontenttype>"));
        assert!(xml.contains("<D:status>HTTP/1.1 200 OK</D:status>"));
        assert!(
            xml.contains("<D:resourcetype/>")
                || xml.contains("<D:resourcetype></D:resourcetype>")
        );
    }

    #[test]
    fn test_dav_directory_xml_serialization() {
        let dir_node = FsNode {
            id: Uuid::new_v4(),
            path: "Documents".to_string(),
            is_dir: true,
            size: None,
            mime_type: None,
            parent_id: None,
        };

        let response = build_dav_response(&dir_node);
        let xml = to_string(&response).expect("failed to serialize directory response");

        assert!(xml.contains("<D:collection/>") || xml.contains("<D:collection></D:collection>"));
    }
}
