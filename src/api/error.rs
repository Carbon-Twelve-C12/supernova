/// Legacy alias for compatibility
pub type ApiResult<T> = Result<T>;

/// Wrapper type for JSON responses
pub struct JsonResponse<T>(pub T);

impl<T: serde::Serialize> Responder for JsonResponse<T> {
    type Body = actix_web::body::BoxBody;

    fn respond_to(self, _req: &HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self.0)
    }
}

/// Helper function to convert ApiResult to a proper response
pub fn to_response<T: serde::Serialize>(result: ApiResult<T>) -> impl Responder {
    match result {
        Ok(data) => HttpResponse::Ok().json(data),
        Err(err) => err.error_response(),
    }
} 