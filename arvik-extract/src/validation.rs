//! Declarative request validation extractors.

use std::collections::BTreeMap;

use arvik_core::extract::{FromRequest, FromRequestParts};
use arvik_core::{IntoResponse, Request, RequestParts, Response, ResponseBuilder};
use http::StatusCode;
use serde::Serialize;
use serde::de::DeserializeOwned;
use validator::{Validate, ValidationErrors, ValidationErrorsKind};

use crate::rejection::{FormRejection, JsonRejection, QueryRejection};
use crate::{Form, Json, Query};

/// JSON extractor that validates the parsed value with `validator`.
#[derive(Debug, Clone)]
pub struct ValidatedJson<T>(pub T);

/// URL-encoded form extractor that validates the parsed value with `validator`.
#[derive(Debug, Clone)]
pub struct ValidatedForm<T>(pub T);

/// Query string extractor that validates the parsed value with `validator`.
#[derive(Debug, Clone)]
pub struct ValidatedQuery<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = ValidationRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(ValidationRejection::Json)?;
        validate_value(value).map(ValidatedJson)
    }
}

impl<S, T> FromRequest<S> for ValidatedForm<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = ValidationRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Form(value) = Form::<T>::from_request(req, state)
            .await
            .map_err(ValidationRejection::Form)?;
        validate_value(value).map(ValidatedForm)
    }
}

impl<S, T> FromRequestParts<S> for ValidatedQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate + Send,
{
    type Rejection = ValidationRejection;

    async fn from_request_parts(
        parts: &mut RequestParts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(ValidationRejection::Query)?;
        validate_value(value).map(ValidatedQuery)
    }
}

fn validate_value<T: Validate>(value: T) -> Result<T, ValidationRejection> {
    value
        .validate()
        .map(|()| value)
        .map_err(|errors| ValidationRejection::Validation(ValidationProblem::from(errors)))
}

/// Rejection returned by validated extractors.
#[derive(Debug)]
pub enum ValidationRejection {
    /// JSON parsing/content-type failed before validation could run.
    Json(JsonRejection),
    /// Form parsing/content-type failed before validation could run.
    Form(FormRejection),
    /// Query parsing failed before validation could run.
    Query(QueryRejection),
    /// Parsed input failed declarative validation.
    Validation(ValidationProblem),
}

impl IntoResponse for ValidationRejection {
    fn into_response(self) -> Response {
        match self {
            Self::Json(rejection) => rejection.into_response(),
            Self::Form(rejection) => rejection.into_response(),
            Self::Query(rejection) => rejection.into_response(),
            Self::Validation(problem) => ResponseBuilder::new()
                .status(StatusCode::UNPROCESSABLE_ENTITY)
                .header(http::header::CONTENT_TYPE, "application/json")
                .json(&problem),
        }
    }
}

impl std::fmt::Display for ValidationRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(rejection) => write!(f, "{rejection}"),
            Self::Form(rejection) => write!(f, "{rejection}"),
            Self::Query(rejection) => write!(f, "{rejection}"),
            Self::Validation(problem) => write!(f, "{}", problem.message),
        }
    }
}

impl std::error::Error for ValidationRejection {}

/// JSON response body for validation failures.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationProblem {
    /// Stable machine-readable error code.
    pub error: &'static str,
    /// Human-readable summary.
    pub message: &'static str,
    /// Field-level validation errors.
    pub fields: Vec<ValidationFieldError>,
}

impl From<ValidationErrors> for ValidationProblem {
    fn from(errors: ValidationErrors) -> Self {
        let mut fields = Vec::new();
        collect_validation_errors("", &errors, &mut fields);
        Self {
            error: "validation_failed",
            message: "Request validation failed",
            fields,
        }
    }
}

/// A sanitized field-level validation error.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationFieldError {
    /// Dot/bracket path to the invalid field.
    pub field: String,
    /// Validator error code, such as `email` or `length`.
    pub code: String,
    /// Optional custom message from the validator attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Validator parameters with raw invalid `value` removed.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, serde_json::Value>,
}

fn collect_validation_errors(
    prefix: &str,
    errors: &ValidationErrors,
    fields: &mut Vec<ValidationFieldError>,
) {
    for (field, kind) in errors.errors() {
        let field_path = join_path(prefix, field);
        match kind {
            ValidationErrorsKind::Field(errors) => {
                for error in errors {
                    let params = error
                        .params
                        .iter()
                        .filter(|(key, _)| key.as_ref() != "value")
                        .map(|(key, value)| (key.to_string(), value.clone()))
                        .collect();
                    fields.push(ValidationFieldError {
                        field: field_path.clone(),
                        code: error.code.to_string(),
                        message: error.message.as_ref().map(ToString::to_string),
                        params,
                    });
                }
            }
            ValidationErrorsKind::Struct(errors) => {
                collect_validation_errors(&field_path, errors, fields);
            }
            ValidationErrorsKind::List(items) => {
                for (index, errors) in items {
                    collect_validation_errors(&format!("{field_path}[{index}]"), errors, fields);
                }
            }
        }
    }
}

fn join_path(prefix: &str, field: &str) -> String {
    if prefix.is_empty() {
        field.to_string()
    } else {
        format!("{prefix}.{field}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arvik_core::Body;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, Validate)]
    struct Signup {
        #[validate(length(min = 2))]
        name: String,
        #[validate(email)]
        email: String,
    }

    #[derive(Debug, Deserialize, Validate)]
    struct Profile {
        #[validate(email)]
        email: String,
    }

    #[derive(Debug, Deserialize, Validate)]
    struct NestedSignup {
        #[validate(nested)]
        profile: Profile,
    }

    fn json_request(body: &str) -> Request {
        Request::new(
            http::Request::builder()
                .method(http::Method::POST)
                .uri("/")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn valid_json_reaches_handler() {
        let ValidatedJson(signup) = ValidatedJson::<Signup>::from_request(
            json_request(r#"{"name":"Darshan","email":"darshan@example.com"}"#),
            &(),
        )
        .await
        .unwrap();

        assert_eq!(signup.name, "Darshan");
    }

    #[tokio::test]
    async fn json_validation_failure_returns_422_with_sanitized_fields() {
        let rejection = ValidatedJson::<Signup>::from_request(
            json_request(r#"{"name":"A","email":"not-an-email"}"#),
            &(),
        )
        .await
        .unwrap_err();

        let response = rejection.into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = response.into_body().to_string().await.unwrap();

        assert!(body.contains(r#""field":"name""#));
        assert!(body.contains(r#""field":"email""#));
        assert!(body.contains(r#""code":"length""#));
        assert!(body.contains(r#""code":"email""#));
        assert!(!body.contains("not-an-email"));
        assert!(!body.contains(r#""value""#));
    }

    #[tokio::test]
    async fn json_parse_failures_keep_existing_statuses() {
        let invalid_json =
            ValidatedJson::<Signup>::from_request(json_request(r#"{"name":"A""#), &())
                .await
                .unwrap_err()
                .into_response();
        assert_eq!(invalid_json.status(), StatusCode::BAD_REQUEST);

        let missing_content_type = Request::new(
            http::Request::builder()
                .method(http::Method::POST)
                .uri("/")
                .body(Body::from("{}"))
                .unwrap(),
        );
        let response = ValidatedJson::<Signup>::from_request(missing_content_type, &())
            .await
            .unwrap_err()
            .into_response();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn nested_validation_preserves_field_path() {
        let rejection = ValidatedJson::<NestedSignup>::from_request(
            json_request(r#"{"profile":{"email":"bad"}}"#),
            &(),
        )
        .await
        .unwrap_err();

        let response = rejection.into_response();
        let body = response.into_body().to_string().await.unwrap();

        assert!(body.contains(r#""field":"profile.email""#));
    }

    #[derive(Debug, Deserialize, Validate)]
    struct Search {
        #[validate(length(min = 2))]
        q: String,
    }

    #[tokio::test]
    async fn validated_query_uses_422_for_validation_failures() {
        let request = Request::new(
            http::Request::builder()
                .uri("/search?q=x")
                .body(Body::empty())
                .unwrap(),
        );
        let (mut parts, _) = request.into_request_parts();
        let rejection = ValidatedQuery::<Search>::from_request_parts(&mut parts, &())
            .await
            .unwrap_err();

        assert_eq!(
            rejection.into_response().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[tokio::test]
    async fn valid_form_reaches_handler() {
        let request = Request::new(
            http::Request::builder()
                .method(http::Method::POST)
                .uri("/")
                .header(
                    http::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(Body::from("name=Darshan&email=darshan%40example.com"))
                .unwrap(),
        );

        let ValidatedForm(signup) = ValidatedForm::<Signup>::from_request(request, &())
            .await
            .unwrap();

        assert_eq!(signup.email, "darshan@example.com");
    }
}
